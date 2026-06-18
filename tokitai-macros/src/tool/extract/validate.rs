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
//! The single exported entry point is [`validate_impl`], which
//! walks the whole impl block and returns the full set of
//! `MacroError`s. Each `MacroError` carries its own span (the
//! user-written token it refers to) and is rendered through
//! `compile_error!` by the proc-macro driver in `tool/mod.rs`,
//! so the user sees the diagnostic at the offending line in
//! their source.

use syn::spanned::Spanned;
use syn::{ImplItem, ImplItemFn, ItemImpl, Pat, Visibility};
// `Spanned` is required for `.span()` calls on `syn::Type` and
// `syn::Attribute` (those types do not expose an inherent
// `span()` method).

use crate::error::{levenshtein, ErrorCode, MacroError};
use crate::tool::schema::dialect::Dialect;

/// Maximum number of user-facing parameters a `#[tool]` method may
/// declare. Past this point the generated JSON Schema becomes
/// unwieldy and most LLM tool-calling backends refuse the call.
pub(crate) const MAX_PARAMS: usize = 32;

/// Run every validation we know how to run on every method in
/// `impl_item` and return the (possibly-empty) list of
/// diagnostics. The proc-macro driver surfaces only the first
/// error from this list (rustc shows one error per macro
/// invocation anyway); the rest are kept around so the snapshot
/// tests can assert on the full set.
///
/// T-012: the impl-level `#[tool(dialect = "...")]` attribute
/// is validated by [`validate_impl_dialect_only`] *before*
/// this function runs, so we do not duplicate the check here.
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
                // E0021 — return type must be a shape the schema
                // generator can lower to JSON Schema. Raw function
                // pointers (`fn(i32) -> i32`), `dyn Trait`, and
                // `impl Trait` are all rejected here so the user
                // gets a clean diagnostic instead of an opaque
                // "type not supported" at the schema call site.
                if let Some(bad) = unsupported_return_type(ty) {
                    errs.push(
                        MacroError::new(
                            ErrorCode::E0021,
                            ty.span(),
                            format!("method `{}` has an unsupported return type: {}", name, bad),
                        )
                        .with_help(
                            "the macro can only lower `String`, primitive numbers, `bool`, \
                             `Option<T>`, `Vec<T>`, `HashMap<K, V>`, structs that derive \
                             `serde::Deserialize`, and `serde_json::Value`; wrap the value in a \
                             concrete type or return `serde_json::Value` and serialize inside the \
                             body",
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
                                    format!("method `{}` is `async` and takes `&mut self`", name),
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

/// Detect return types the JSON-Schema generator cannot lower.
///
/// The macro lowers return types to a JSON Schema for the
/// `output_schema` of each tool. The following shapes are not
/// representable in JSON Schema and would otherwise explode at
/// the schema call site with an opaque "type not supported"
/// message. The check returns a human-readable name of the
/// offending shape (used as the diagnostic detail) or `None`
/// when the type is OK.
///
/// This is intentionally conservative: it only flags shapes
/// that are *known* to be unschemaable. Types the schema
/// generator handles via opaque `Value` fall-through (e.g. a
/// user struct that derives `Deserialize`) are not flagged.
fn unsupported_return_type(ty: &syn::Type) -> Option<&'static str> {
    match ty {
        // `fn(i32) -> i32` — bare function pointer, not a JSON value.
        syn::Type::BareFn(_) => Some("bare function pointer `fn(...) -> ...`"),
        // `dyn Trait` — not a JSON value.
        syn::Type::TraitObject(_) => Some("`dyn Trait` object"),
        // `impl Trait` — the macro cannot name the underlying type
        // to embed it in a JSON Schema.
        syn::Type::ImplTrait(_) => Some("`impl Trait` return"),
        // `&'static T`, `&mut T` — borrowed values the tool
        // dispatcher cannot return by-value to the LLM caller.
        syn::Type::Reference(_) => Some("borrowed reference (`&T` / `&mut T`)"),
        // Raw pointers — same reason as references.
        syn::Type::Ptr(_) => Some("raw pointer (`*const T` / `*mut T`)"),
        _ => None,
    }
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

/// T-012: Validate the impl-level `#[tool(dialect = "...")]`
/// attribute (if any). Unknown dialect names produce a clean
/// `E0030` diagnostic anchored at the offending attribute, so
/// the user is told which dialect name they tried to use.
///
/// The audit of every emitted `ToolDefinition.input_schema`
/// happens later (in `codegen::definitions`) once the schema
/// has been rendered; this function only validates the
/// attribute's *name*.
///
/// `pub(crate)` so `tool/mod.rs` can call it before the
/// empty-impl short-circuit (the dialect check must run
/// even when the impl has no active methods).
pub(crate) fn validate_impl_dialect_only(impl_item: &ItemImpl) -> Option<MacroError> {
    for attr in &impl_item.attrs {
        if !attr.path().is_ident("tool") {
            continue;
        }
        // Try the lightweight `MiniToolAttrs` parser first; if
        // it fails (because the attribute has a shape the
        // minimal parser doesn't handle) we let the main
        // `ToolAttributes` parser surface it elsewhere.
        if let Ok(parsed) = attr.parse_args::<MiniImplAttrs>() {
            if let Some(name) = parsed.dialect {
                if Dialect::from_name(&name).is_none() {
                    return Some(
                        MacroError::new(
                            ErrorCode::E0030,
                            attr.span(),
                            format!(
                                "unknown schema dialect `{}` in `#[tool(dialect = \"...\")]`",
                                name
                            ),
                        )
                        .with_help(
                            "supported dialects: `mcp`, `openai-strict`, `anthropic` \
                             (default is `mcp` if `dialect` is omitted)"
                                .to_string(),
                        ),
                    );
                }
            }
        }
    }
    None
}

/// Minimal subset of impl-level `ToolAttributes` parser. We
/// only care about `dialect = "..."` here; other keys are
/// dropped on the floor so the parse doesn't fail.
struct MiniImplAttrs {
    dialect: Option<String>,
}

impl syn::parse::Parse for MiniImplAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut dialect = None;
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;
            match key.to_string().as_str() {
                "dialect" => {
                    let v: syn::LitStr = input.parse()?;
                    dialect = Some(v.value());
                }
                _ => {
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
        Ok(MiniImplAttrs { dialect })
    }
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
    fn detects_unknown_dialect_name() {
        // T-012: `#[tool(dialect = "garbage")]` is rejected
        // at the impl-block attribute span.
        let item: ItemImpl = parse_quote! {
            #[tool(dialect = "garbage")]
            impl Foo {
                pub fn ok(&self) -> i32 { 1 }
            }
        };
        let err = validate_impl_dialect_only(&item);
        assert!(err.is_some(), "expected unknown-dialect diagnostic");
        let err = err.unwrap();
        assert_eq!(err.code(), ErrorCode::E0030);
        assert!(err.to_diagnostic().contains("garbage"));
    }

    #[test]
    fn accepts_known_dialect_name() {
        let item: ItemImpl = parse_quote! {
            #[tool(dialect = "openai-strict")]
            impl Foo {
                pub fn ok(&self) -> i32 { 1 }
            }
        };
        assert!(validate_impl_dialect_only(&item).is_none());
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

    #[test]
    fn span_is_user_written_token_for_missing_self() {
        // T-001: the diagnostic must surface at the method's
        // identifier span (the user-written token), not at the
        // macro call site. We assert on the `MacroError::span()`
        // directly: comparing spans as `Span` values is unreliable
        // in unit-test mode, but the diagnostic body still
        // references the method name.
        let item: ItemImpl = parse_quote! {
            impl Foo {
                pub fn method_without_self(a: i32) -> i32 { a }
            }
        };
        let errs = validate_impl(&item);
        assert_eq!(errs.len(), 1);
        let body = errs[0].to_diagnostic_body();
        assert!(
            body.contains("method_without_self"),
            "diagnostic must reference user-written method name, got:\n{}",
            body
        );
    }
}
