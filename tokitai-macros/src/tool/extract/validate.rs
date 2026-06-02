//! Validation pipeline for `#[tool]` and friends.
//!
//! Runs *after* `extract_tool_info` has built the raw
//! `ToolMethodInfo` slice, but *before* the codegen layer is given
//! control. Anything wrong with the user's source code that the
//! extractor can statically detect is flagged here with a polished
//! [`MacroError`] (stable code + span + suggestion) so the user
//! sees the same diagnostic quality regardless of which macro
//! tripped the rule.
//!
//! Two surfaces are exported:
//!
//! - [`validate_tool_method`] checks a single method and returns
//!   `syn::Result<()>`; the first failure short-circuits. The
//!   proc-macro driver in `tool/mod.rs` calls this per method.
//! - [`validate_impl`] walks the whole impl and returns a
//!   `Vec<MacroError>`, which is what the `trybuild` snapshot
//!   tests use so they can assert the *full* set of diagnostics
//!   in one fixture.

use syn::spanned::Spanned;
use syn::{ImplItem, ImplItemFn, ItemImpl, Pat, Visibility};

use crate::error::{format_did_you_mean, levenshtein, suggest_closest, ErrorCode, MacroError};

/// Maximum number of user-facing parameters a `#[tool]` method may
/// declare. Past this point the generated JSON Schema becomes
/// unwieldy and most LLM tool-calling backends refuse the call.
pub(crate) const MAX_PARAMS: usize = 32;

/// Validate a single method.
///
/// Returns `Ok(())` if the method passes every check, or the first
/// `syn::Error` (derived from a [`MacroError`]) that tripped the
/// validator. We return `syn::Error` rather than `MacroError`
/// directly because the proc-macro driver's `compile_error!`
/// pipeline is built around `syn::Error`.
pub(crate) fn validate_tool_method(fn_item: &ImplItemFn) -> syn::Result<()> {
    if !is_public(fn_item) {
        return Ok(());
    }
    let name = fn_item.sig.ident.to_string();

    // E0022 — `__`-prefixed method names are reserved by the
    // macro. We check this *before* the early return below so the
    // user sees a useful error rather than a silent skip.
    if name.starts_with("__") {
        return Err(macro_to_syn(
            MacroError::new(
                ErrorCode::E0022,
                fn_item.sig.ident.span(),
                format!(
                    "method `{}` starts with `__`, which is reserved by the macro",
                    name
                ),
            )
            .with_help(
                "rename the method: the macro emits `__TOOL_DEF_*`, `__call_*`, \
                 `__get_tool_definitions`, `__TOOL_COUNT`, and `__OPENAPI_*` items; \
                 a user method that starts with `__` would shadow one of them",
            ),
        ));
    }

    // E0012 — every tool method must have a `self` parameter.
    if !has_self_receiver(fn_item) {
        return Err(macro_to_syn(
            MacroError::new(
                ErrorCode::E0012,
                fn_item.sig.ident.span(),
                format!("method `{}` has no `self` parameter", name),
            )
            .with_help(
                "add a `self` parameter (`&self` for read-only, `&mut self` to mutate, \
                 `self` to consume); the macro generates a tool dispatcher that needs a \
                 receiver to call into",
            ),
        ));
    }

    // E0004 — no generic parameters.
    if !fn_item.sig.generics.params.is_empty() {
        return Err(macro_to_syn(
            MacroError::new(
                ErrorCode::E0004,
                fn_item.sig.ident.span(),
                format!("method `{}` uses generic parameters", name),
            )
            .with_help(
                "use a concrete type in the signature, or take a `serde_json::Value` and \
                 deserialize inside the body; the macro cannot generate a single typed \
                 schema for an open-ended `T: ...`",
            ),
        ));
    }

    // E0005 — no `Self` in return type.
    if let syn::ReturnType::Type(_, ty) = &fn_item.sig.output {
        if type_mentions_self(ty) {
            return Err(macro_to_syn(
                MacroError::new(
                    ErrorCode::E0005,
                    fn_item.sig.ident.span(),
                    format!("method `{}` returns `Self`", name),
                )
                .with_help(
                    "replace `Self` with the concrete type, or with `serde_json::Value` and \
                     serialize the value inside the body; the schema generator cannot \
                     materialise `Self` without seeing the full type definition",
                ),
            ));
        }
    }

    // E0006 — no `async` + `&mut self`.
    // E0024 — no `async` + consuming `self`.
    if fn_item.sig.asyncness.is_some() {
        for arg in &fn_item.sig.inputs {
            if let syn::FnArg::Receiver(r) = arg {
                if r.mutability.is_some() {
                    return Err(macro_to_syn(
                        MacroError::new(
                            ErrorCode::E0006,
                            r.colon_token.span(),
                            format!("method `{}` is `async` and takes `&mut self`", name),
                        )
                        .with_help(
                            "use `&self` (or `&mut self` *without* `async`); the async \
                             executor cannot guarantee exclusive access to `self` while \
                             the future is suspended",
                        ),
                    ));
                }
                if r.reference.is_none() && r.mutability.is_none() {
                    return Err(macro_to_syn(
                        MacroError::new(
                            ErrorCode::E0024,
                            r.span(),
                            format!("method `{}` is `async` and consumes `self`", name),
                        )
                        .with_help(
                            "use `&self` (read-only) or `&mut self` (in-place mutation); \
                             an `async` method that consumes `self` cannot be dispatched \
                             because the receiver is gone by the time the future resolves",
                        ),
                    ));
                }
            }
        }
    }

    // E0025 — no `unsafe` methods.
    if let Some(tok) = fn_item.sig.unsafety {
        return Err(macro_to_syn(
            MacroError::new(
                ErrorCode::E0025,
                tok.span,
                format!("method `{}` is `unsafe`", name),
            )
            .with_help(
                "remove `unsafe` from the signature (the macro-generated wrapper is a safe \
                 function, so propagating `unsafe` is not possible), or call the unsafe code \
                 from a safe wrapper and mark *that* as the tool method",
            ),
        ));
    }

    // E0026 — no trait default methods (heuristic via `#[default]` attr).
    for attr in &fn_item.attrs {
        if attr.path().is_ident("default") {
            return Err(macro_to_syn(
                MacroError::new(
                    ErrorCode::E0026,
                    attr.span(),
                    format!("method `{}` is a trait default method", name),
                )
                .with_help(
                    "move the body of the default method into a regular `impl` block, or \
                     annotate the trait method directly with `#[tool]` instead of the \
                     default body",
                ),
            ));
        }
    }

    // E0027 — param count <= MAX_PARAMS.
    let user_params = fn_item
        .sig
        .inputs
        .iter()
        .filter(|arg| match arg {
            syn::FnArg::Receiver(_) => false,
            syn::FnArg::Typed(pat_type) => !is_self_pat(&pat_type.pat),
        })
        .count();
    if user_params > MAX_PARAMS {
        return Err(macro_to_syn(
            MacroError::new(
                ErrorCode::E0027,
                fn_item.sig.ident.span(),
                format!(
                    "method `{}` has {} parameters (limit: {})",
                    name, user_params, MAX_PARAMS
                ),
            )
            .with_help(
                "split the method into smaller ones (each with a focused responsibility), \
                 or group related parameters into a struct that the tool takes as a single \
                 argument",
            ),
        ));
    }

    // E0028 — name/alias conflict.
    if let Some(conflict) = name_alias_conflict(fn_item) {
        return Err(macro_to_syn(
            MacroError::new(
                ErrorCode::E0028,
                fn_item.sig.ident.span(),
                format!(
                    "method `{}` has `name = \"{}\"` and an alias also set to `\"{}\"`",
                    name, conflict, conflict
                ),
            )
            .with_help(
                "remove the alias, change the alias to a different string, or change the \
                 `name = \"...\"` so the two no longer collide",
            ),
        ));
    }

    Ok(())
}

/// Run every validation we know how to run on every method in
/// `impl_item` and return the (possibly-empty) list of
/// diagnostics. The proc-macro driver surfaces only the first
/// error from this list (rustc shows one error per macro
/// invocation anyway); the rest are kept around so the snapshot
/// tests can assert on the full set.
pub(crate) fn validate_impl(impl_item: &ItemImpl) -> Vec<MacroError> {
    let mut errs = Vec::new();
    for item in &impl_item.items {
        if let ImplItem::Fn(fn_item) = item {
            if !is_public(fn_item) {
                continue;
            }
            let name = fn_item.sig.ident.to_string();
            if name.starts_with("__") {
                continue;
            }
            // Re-run each rule individually so we can report every
            // problem with the right code, not just the first.
            if !has_self_receiver(fn_item) {
                errs.push(
                    MacroError::new(
                        ErrorCode::E0012,
                        fn_item.sig.ident.span(),
                        format!("method `{}` has no `self` parameter", name),
                    )
                    .with_help(
                        "add a `self` parameter (`&self` for read-only, `&mut self` to mutate, \
                         `self` to consume); the macro generates a tool dispatcher that needs a \
                         receiver to call into",
                    ),
                );
            }
            if !fn_item.sig.generics.params.is_empty() {
                errs.push(
                    MacroError::new(
                        ErrorCode::E0004,
                        fn_item.sig.ident.span(),
                        format!("method `{}` uses generic parameters", name),
                    )
                    .with_help(
                        "use a concrete type in the signature, or take a `serde_json::Value` and \
                         deserialize inside the body; the macro cannot generate a single typed \
                         schema for an open-ended `T: ...`",
                    ),
                );
            }
            if let syn::ReturnType::Type(_, ty) = &fn_item.sig.output {
                if type_mentions_self(ty) {
                    errs.push(
                        MacroError::new(
                            ErrorCode::E0005,
                            fn_item.sig.ident.span(),
                            format!("method `{}` returns `Self`", name),
                        )
                        .with_help(
                            "replace `Self` with the concrete type, or with `serde_json::Value` \
                             and serialize the value inside the body; the schema generator \
                             cannot materialise `Self` without seeing the full type definition",
                        ),
                    );
                }
            }
            if fn_item.sig.asyncness.is_some() {
                for arg in &fn_item.sig.inputs {
                    if let syn::FnArg::Receiver(r) = arg {
                        if r.mutability.is_some() {
                            errs.push(
                                MacroError::new(
                                    ErrorCode::E0006,
                                    r.colon_token.span(),
                                    format!(
                                        "method `{}` is `async` and takes `&mut self`",
                                        name
                                    ),
                                )
                                .with_help(
                                    "use `&self` (or `&mut self` *without* `async`); the async \
                                     executor cannot guarantee exclusive access to `self` while \
                                     the future is suspended",
                                ),
                            );
                        }
                        if r.reference.is_none() && r.mutability.is_none() {
                            errs.push(
                                MacroError::new(
                                    ErrorCode::E0024,
                                    r.span(),
                                    format!("method `{}` is `async` and consumes `self`", name),
                                )
                                .with_help(
                                    "use `&self` (read-only) or `&mut self` (in-place \
                                     mutation); an `async` method that consumes `self` cannot \
                                     be dispatched because the receiver is gone by the time \
                                     the future resolves",
                                ),
                            );
                        }
                    }
                }
            }

            // E0025 — unsafe.
            if let Some(tok) = fn_item.sig.unsafety {
                errs.push(
                    MacroError::new(
                        ErrorCode::E0025,
                        tok.span,
                        format!("method `{}` is `unsafe`", name),
                    )
                    .with_help(
                        "remove `unsafe` from the signature (the macro-generated wrapper is \
                         a safe function, so propagating `unsafe` is not possible), or call \
                         the unsafe code from a safe wrapper and mark *that* as the tool method",
                    ),
                );
            }

            // E0026 — trait default method.
            for attr in &fn_item.attrs {
                if attr.path().is_ident("default") {
                    errs.push(
                        MacroError::new(
                            ErrorCode::E0026,
                            attr.span(),
                            format!("method `{}` is a trait default method", name),
                        )
                        .with_help(
                            "move the body of the default method into a regular `impl` block, \
                             or annotate the trait method directly with `#[tool]` instead of \
                             the default body",
                        ),
                    );
                }
            }

            // E0027 — param count.
            let user_params = fn_item
                .sig
                .inputs
                .iter()
                .filter(|arg| match arg {
                    syn::FnArg::Receiver(_) => false,
                    syn::FnArg::Typed(pat_type) => !is_self_pat(&pat_type.pat),
                })
                .count();
            if user_params > MAX_PARAMS {
                errs.push(
                    MacroError::new(
                        ErrorCode::E0027,
                        fn_item.sig.ident.span(),
                        format!(
                            "method `{}` has {} parameters (limit: {})",
                            name, user_params, MAX_PARAMS
                        ),
                    )
                    .with_help(
                        "split the method into smaller ones (each with a focused responsibility), \
                         or group related parameters into a struct that the tool takes as a single \
                         argument",
                    ),
                );
            }

            // E0028 — name/alias conflict.
            if let Some(conflict) = name_alias_conflict(fn_item) {
                errs.push(
                    MacroError::new(
                        ErrorCode::E0028,
                        fn_item.sig.ident.span(),
                        format!(
                            "method `{}` has `name = \"{}\"` and an alias also set to `\"{}\"`",
                            name, conflict, conflict
                        ),
                    )
                    .with_help(
                        "remove the alias, change the alias to a different string, or change \
                         the `name = \"...\"` so the two no longer collide",
                    ),
                );
            }
        }
    }
    errs
}

fn is_public(fn_item: &ImplItemFn) -> bool {
    matches!(fn_item.vis, Visibility::Public(_))
}

fn has_self_receiver(fn_item: &ImplItemFn) -> bool {
    for arg in &fn_item.sig.inputs {
        if let syn::FnArg::Receiver(_) = arg {
            return true;
        }
    }
    false
}

fn type_mentions_self(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        if p.qself.is_none() {
            if let Some(last) = p.path.segments.last() {
                if last.ident == "Self" {
                    return true;
                }
            }
        }
    }
    false
}

/// Return `true` if the given pattern is the literal `self` ident.
fn is_self_pat(pat: &Pat) -> bool {
    if let Pat::Ident(pi) = pat {
        return pi.ident == "self";
    }
    false
}

/// Inspect the `#[tool(name = "...", alias = ["..."])]` attribute
/// and return the colliding string if `name` and an entry in
/// `alias` are identical. Returns `None` when no collision exists
/// (or the attribute is missing or malformed; that case is handled
/// by the main `MethodToolAttrs` parser).
fn name_alias_conflict(fn_item: &ImplItemFn) -> Option<String> {
    let mut name: Option<String> = None;
    let mut aliases: Vec<String> = Vec::new();
    for attr in &fn_item.attrs {
        if !attr.path().is_ident("tool") {
            continue;
        }
        // Best-effort parse: if the user wrote a syntactically
        // broken attribute we let the regular parser surface it.
        if let Ok(parsed) = attr.parse_args::<MiniToolAttrs>() {
            if let Some(n) = parsed.name {
                name = Some(n);
            }
            aliases.extend(parsed.alias);
        }
    }
    let n = name?;
    for a in &aliases {
        if a == &n {
            return Some(a.clone());
        }
    }
    None
}

/// Lightweight stand-in for `MethodToolAttrs` so we can pull just
/// `name` and `alias` from the attribute without duplicating the
/// full parse tree. The full parser lives in `attrs/method.rs`;
/// this minimal parser tolerates any other keys (it just drops
/// their values) so the attribute parse doesn't fail on this
/// side before the main parser has a chance to run.
struct MiniToolAttrs {
    name: Option<String>,
    alias: Vec<String>,
}

impl syn::parse::Parse for MiniToolAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut alias = Vec::new();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;
            match key.to_string().as_str() {
                "name" => {
                    let v: syn::LitStr = input.parse()?;
                    name = Some(v.value());
                }
                "alias" => {
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let v: syn::LitStr = content.parse()?;
                        alias.push(v.value());
                        if content.peek(syn::Token![,]) {
                            content.parse::<syn::Token![,]>()?;
                        }
                    }
                }
                _ => {
                    // Swallow any value to keep the parse moving.
                    if input.peek(syn::token::Bracket) {
                        let _content;
                        let _ = syn::bracketed!(_content in input);
                    } else {
                        let _: syn::LitStr = input.parse()?;
                    }
                }
            }
            if input.peek(syn::Token![,]) {
                input.parse::<syn::Token![,]>()?;
            }
        }
        Ok(MiniToolAttrs { name, alias })
    }
}

/// Convert a [`MacroError`] to a `syn::Error` so the proc-macro
/// driver can flow it through its existing `to_compile_error`
/// pipeline. The formatted diagnostic is used as the user-facing
/// message; rustc's own location is the macro's call site, which
/// the `compile_error!` macro picks up automatically.
///
/// We use the *body* form (no leading `error:`) so the
/// `compile_error!` wrapper does not produce a doubled
/// `error: error[Exxxx]: ...` line. The body's `E0xxx: ...`
/// prefix is the only place that identifier appears in the
/// rendered output.
fn macro_to_syn(e: MacroError) -> syn::Error {
    syn::Error::new(proc_macro2::Span::call_site(), e.to_diagnostic_body())
}

/// Convert a [`MacroError`] directly to a `TokenStream` that emits
/// the diagnostic via `compile_error!`. We use this in the
/// proc-macro driver so the rendered output goes through our
/// own `to_compile_error` (which writes the same `compile_error!`
/// invocation as `MacroError::to_compile_error`, but the macro
/// entry point in `lib.rs` still re-channels it through
/// `syn::Error` because the rest of the macro pipeline returns
/// `syn::Error`).
fn macro_to_tokens(e: MacroError) -> proc_macro2::TokenStream {
    e.to_compile_error()
}

// ---------------------------------------------------------------------------
// Re-export of "did-you-mean" for the `#[wrap]` driver
// ---------------------------------------------------------------------------

/// Return a "did you mean" suggestion string (or `None` if no
/// candidate is close enough) for the case where a `methods = [...]`
/// entry in `#[wrap(...)]` is not present in the impl block.
pub(crate) fn suggest_method_name(requested: &str, available: &[String]) -> Option<String> {
    let sugg = suggest_closest(requested, available);
    format_did_you_mean(&sugg)
}

#[allow(dead_code)]
fn _levenshtein_reexport_for_tests(a: &str, b: &str) -> usize {
    levenshtein(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn detects_missing_self() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub fn bad(a: i32) -> i32 { a }
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), ErrorCode::E0012);
        assert!(errs[0].to_diagnostic().contains("E0012"));
    }

    #[test]
    fn detects_generic() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub fn bad<T>(&self, x: T) -> T { x }
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), ErrorCode::E0004);
    }

    #[test]
    fn detects_self_return() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub fn bad(&self) -> Self { unimplemented!() }
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), ErrorCode::E0005);
    }

    #[test]
    fn detects_async_mut_self() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub async fn bad(&mut self) {}
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), ErrorCode::E0006);
    }

    #[test]
    fn detects_async_consuming_self() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub async fn bad(self) {}
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), ErrorCode::E0024);
    }

    #[test]
    fn detects_unsafe() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub unsafe fn bad(&self) {}
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), ErrorCode::E0025);
    }

    #[test]
    fn detects_default_method() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                #[default]
                pub fn bad(&self) -> i32 { 1 }
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), ErrorCode::E0026);
    }

    #[test]
    fn detects_too_many_params() {
        // 33 typed parameters: p0..p32.
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub fn bad(&self, p0: i32, p1: i32, p2: i32, p3: i32, p4: i32,
                            p5: i32, p6: i32, p7: i32, p8: i32, p9: i32,
                            p10: i32, p11: i32, p12: i32, p13: i32, p14: i32,
                            p15: i32, p16: i32, p17: i32, p18: i32, p19: i32,
                            p20: i32, p21: i32, p22: i32, p23: i32, p24: i32,
                            p25: i32, p26: i32, p27: i32, p28: i32, p29: i32,
                            p30: i32, p31: i32, p32: i32) -> i32 { 0 }
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), ErrorCode::E0027);
    }

    #[test]
    fn detects_name_alias_conflict() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                #[tool(name = "do_it", alias = ["do_it"])]
                pub fn do_it(&self) -> i32 { 1 }
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), ErrorCode::E0028);
    }

    #[test]
    fn does_not_fire_on_passing_impl() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub fn good(&self) -> i32 { 1 }
                pub async fn also_good(&self) -> String { "x".into() }
            }
        };
        assert!(validate_impl(&item).is_empty());
    }

    #[test]
    fn validate_tool_method_first_fail_wins() {
        // missing self, generic, returns Self, async + &mut self:
        // we expect *some* error; the exact code depends on the
        // ordering of the rules. Document that here.
        let item: ImplItemFn = parse_quote! {
            pub fn bad<T: ToString>(a: T) -> Self { unimplemented!() }
        };
        let err = validate_tool_method(&item).unwrap_err();
        let msg = err.to_string();
        // E0012 fires first because it is the cheapest check.
        assert!(msg.contains("E0012"), "got: {}", msg);
    }

    #[test]
    fn validate_tool_method_skips_private() {
        let item: ImplItemFn = parse_quote! {
            fn invisible(a: i32) -> i32 { a }
        };
        assert!(validate_tool_method(&item).is_ok());
    }

    #[test]
    fn validate_tool_method_flags_reserved() {
        // `__`-prefixed names now emit E0022 (they are reserved by the
        // macro). Previously the function silently skipped them; we
        // changed the contract so the user sees a clear error.
        let item: ImplItemFn = parse_quote! {
            pub fn __macro_generated(&self) {}
        };
        let err = validate_tool_method(&item).unwrap_err();
        assert!(err.to_string().contains("E0022"), "got: {}", err);
    }

    #[test]
    fn suggest_works_for_typos() {
        let avail = vec!["get_user".to_string(), "list_repos".to_string()];
        assert!(suggest_method_name("getuser", &avail)
            .unwrap()
            .contains("get_user"));
    }

    #[test]
    fn suggest_returns_none_for_unrelated() {
        let avail = vec!["get_user".to_string()];
        let s = suggest_method_name("totally_unrelated_name", &avail);
        assert!(s.is_none());
    }

    #[test]
    fn levenshtein_handles_empty() {
        assert_eq!(_levenshtein_reexport_for_tests("", "abc"), 3);
        assert_eq!(_levenshtein_reexport_for_tests("abc", ""), 3);
    }

    #[test]
    fn diagnostic_is_deterministic_across_calls() {
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub fn bad<T>(&self, x: T) -> T { x }
            }
        };
        let a = validate_impl(&item);
        let b = validate_impl(&item);
        assert_eq!(a[0].to_diagnostic(), b[0].to_diagnostic());
    }
}
