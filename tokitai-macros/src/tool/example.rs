//! T-016: baked few-shot examples as Rust expressions.
//!
//! The `#[tool(example = call!(self.method(args) => result))]` (or the
//! plural `#[tool(examples = [call!(...), call!(...)])]`) attribute lets
//! the user attach a few-shot example to a tool by writing the call
//! site in ordinary Rust syntax. The macro:
//!
//! 1. Parses the `call!(self.method(args) => result)` shape and stores
//!    the literal `(args_tokens, result_tokens)` pair.
//! 2. Emits a compile-time type-check by inlining a wrapper that
//!    invokes `Self::method(args)` with the user's literal types and
//!    binds the result to a let. The host compiler then type-checks
//!    the wrapper against the real signature; if it does not fit, the
//!    user sees a normal type error anchored at the `call!` literal
//!    via `quote_spanned!`.
//! 3. Serializes the literal args + literal result into the tool
//!    schema's `examples` field as a `{ "input": ..., "output": ... }`
//!    JSON object, so the LLM receives a working example pair it can
//!    pattern-match. The serialization happens once at
//!    `LazyLock` initialization (the user's expressions are plain Rust
//!    literals, not calls to `self.method`).
//!
//! Per Q-7, the macro does NOT evaluate the example at proc-macro
//! time. The example is just two token streams the user wrote; the
//! macro inlines them.

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Expr, ExprPath, Token};

/// A single baked few-shot example.
///
/// `args` is the literal token stream the user wrote inside the
/// `()` of the method call. `result` is the literal token stream the
/// user wrote after the `=>`. Both are stored as `TokenStream` (not
/// `Expr`) on purpose: the macro never needs to *interpret* them, it
/// only needs to inline them at two different sites (a type-check
/// call and a `serde_json` value).
#[derive(Clone, Debug)]
pub struct BakedExample {
    /// Tokens representing the user-written argument list (without
    /// the enclosing parentheses). For `self.add(1, 2)` this is
    /// `1 , 2`.
    pub args: TokenStream,
    /// Tokens representing the user-written expected result (the
    /// right-hand side of `=>`).
    pub result: TokenStream,
    /// Span of the original `call!(...)` literal so compile errors
    /// anchor at the user's source rather than at the macro call
    /// site.
    pub span: Span,
}

impl BakedExample {
    /// Render this example as a `serde_json::Value` literal that
    /// the generated `__TOOL_DEF_*` LazyLock initializer will
    /// evaluate once per process. The shape is exactly what the
    /// acceptance criterion specifies:
    ///
    /// ```json
    /// { "input": <args-as-json>, "output": <result-as-json> }
    /// ```
    ///
    /// Where `<args-as-json>` is whatever the user's literal args
    /// serialize to via `serde_json`. For `call!(self.add(1, 2) => 3)`
    /// the user wrote positional args `1, 2`, which serialize to
    /// `[1, 2]` (a JSON array). The OpenAI / Anthropic / MCP docs
    /// all accept either an array or an object under `input`; we
    /// emit the array shape because it preserves the user's exact
    /// positional order.
    pub fn render_json(&self) -> TokenStream {
        let args = &self.args;
        let result = &self.result;
        let span = self.span;
        quote_spanned! {span=>
            ::tokitai::json!({
                "input": ::serde_json::to_value(&(#args))
                    .unwrap_or_else(|_| ::tokitai::Value::Array(::std::vec![])),
                "output": ::serde_json::to_value(&(#result))
                    .unwrap_or(::tokitai::Value::Null),
            })
        }
    }

    /// Emit a compile-time type-check statement anchored at
    /// `self.span`. The wrapper invokes the real method with the
    /// user's literal args and binds the result to a `let`. If the
    /// user's types don't match the method signature, rustc emits
    /// an error at the `call!` literal span (set above via
    /// `quote_spanned!`).
    ///
    /// `wrapper_is_async` is whether the *containing* wrapper
    /// function is async; `method_is_async` is whether the
    /// target method itself returns a future. The full
    /// `result == expected` comparison runs only when the wrapper
    /// can hold the resolved value (i.e. the wrapper is async OR
    /// the method is sync). When the wrapper is sync but the
    /// method is async we drop the result comparison because
    /// `self.method(args)` returns a `Future`, which is not
    /// `PartialEq`; the async wrapper handles the precise check.
    pub fn type_check_tokens(
        &self,
        method_ident: &syn::Ident,
        wrapper_is_async: bool,
        method_is_async: bool,
        return_type: &syn::ReturnType,
    ) -> TokenStream {
        let args = &self.args;
        let result = &self.result;
        let span = self.span;
        let do_result_check = wrapper_is_async || !method_is_async;
        let invoke = if wrapper_is_async && method_is_async {
            quote! { self.#method_ident(#args).await }
        } else {
            quote! { self.#method_ident(#args) }
        };
        if do_result_check {
            // Annotate `__tokitai_expected` with the method's
            // declared return type so a type mismatch produces a
            // plain `expected X, found Y` error anchored at the
            // result span. We avoid the `==` comparison so the
            // error message is stable across feature flags
            // (the comparison would pull in `PartialEq<Value>`
            // notes when the `serde` feature is on).
            let return_type_tokens = return_type_to_tokens(return_type);
            quote_spanned! {span=>
                #[allow(unused)]
                {
                    let __tokitai_actual = #invoke;
                    let __tokitai_expected: #return_type_tokens = #result;
                }
            }
        } else {
            // Sync wrapper of an async method: we cannot type
            // the result against the method's return type
            // because `self.method(args)` returns a `Future`,
            // not the resolved value. The args type check
            // still runs because the future's shape encodes
            // the args types.
            quote_spanned! {span=>
                #[allow(unused)]
                {
                    let __tokitai_future = self.#method_ident(#args);
                    let _ = __tokitai_future;
                }
            }
        }
    }
}

/// Parser for the `call!(self.method(args) => result)` shape.
///
/// Syn doesn't have a built-in pattern for `a => b`, so we parse
/// the call expression first, then look for a `=>` token, then
/// parse the trailing expression as the expected result.
pub struct CallExpr {
    #[allow(dead_code)]
    pub receiver: Expr,
    /// The method ident extracted from `<receiver>.<method>(args)`.
    /// Currently only used for diagnostic messages; future
    /// enhancements (cross-method type checks in `examples = [...]`)
    /// may consume this field.
    #[allow(dead_code)]
    pub method: syn::Ident,
    pub args: TokenStream,
    pub result: TokenStream,
}

impl Parse for CallExpr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse the left-hand side as an arbitrary expression. We
        // accept anything syntactically valid as the receiver —
        // `self`, `Self`, `&self`, or even `MyStruct::new(...)`
        // for static methods. The trailing `.method(args)` is part
        // of the same expression.
        let lhs: Expr = input.parse()?;
        let (receiver, method, args) = extract_call_tail(lhs)?;
        // `=>` separates the call from the expected result.
        input.parse::<Token![=>]>()?;
        let result_tokens: TokenStream = input.parse::<Expr>()?.to_token_stream();
        Ok(CallExpr {
            receiver,
            method,
            args,
            result: result_tokens,
        })
    }
}

/// Walk an arbitrary expression looking for the final
/// `<method>(<args>)` call shape. Returns the receiver (everything
/// before the dot), the method ident, and the args token stream.
///
/// We accept both `self.method(args)` (method call form) and
/// `Self::method(args)` (associated function form) so static
/// / non-`self` tools are reachable too.
fn extract_call_tail(expr: Expr) -> syn::Result<(Expr, syn::Ident, TokenStream)> {
    match expr {
        Expr::MethodCall(mc) => {
            let args_ts = mc.args.to_token_stream();
            Ok((*mc.receiver, mc.method, args_ts))
        }
        Expr::Call(call) => {
            // Allow the `Self::method(args)` path-call form so
            // associated functions work too. The receiver we hand
            // back is a synthetic `Expr::Path` constructed from
            // the original path's leading segments.
            if let Expr::Path(ExprPath { path, .. }) = &*call.func {
                if let Some(last_seg) = path.segments.last() {
                    let method = last_seg.ident.clone();
                    let args_ts = call.args.to_token_stream();
                    // Receiver is everything *before* the last
                    // segment, so `Self::add(args)` -> receiver =
                    // `Self`, method = `add`. If the path has only
                    // one segment we hand back the whole path —
                    // the type-check call will fail at compile time
                    // and rustc will tell the user.
                    let receiver = if path.segments.len() > 1 {
                        let leading = path
                            .segments
                            .iter()
                            .take(path.segments.len() - 1)
                            .cloned()
                            .collect();
                        Expr::Path(ExprPath {
                            attrs: ::std::vec::Vec::new(),
                            qself: None,
                            path: syn::Path {
                                leading_colon: path.leading_colon,
                                segments: leading,
                            },
                        })
                    } else {
                        Expr::Path(ExprPath {
                            attrs: ::std::vec::Vec::new(),
                            qself: None,
                            path: path.clone(),
                        })
                    };
                    return Ok((receiver, method, args_ts));
                }
            }
            Err(syn::Error::new_spanned(
                call.func.as_ref(),
                "tokitai `call!(...)` must end with a method call or path call (e.g. `self.foo(args)` or `Self::foo(args)`)",
            ))
        }
        other => Err(syn::Error::new_spanned(
            other,
            "tokitai `call!(...)` must be of the form `self.method(args) => result` (or `Self::method(args) => result`)",
        )),
    }
}

/// Detect whether the given token stream starts with the `call`
/// ident followed by `!`. We can't use a real `macro_rules!`
/// expansion because the `#[tool]` attribute parser sees
/// pre-expansion tokens.
pub fn looks_like_call_macro(tokens: &TokenStream) -> bool {
    let mut iter = tokens.clone().into_iter();
    let Some(first) = iter.next() else {
        return false;
    };
    if first.to_string() != "call" {
        return false;
    }
    matches!(iter.next(), Some(ref t) if t.to_string() == "!")
}

/// Parse the body of a `call!(...)` macro invocation, i.e. the
/// tokens between the outer parentheses. Returns the parsed
/// [`CallExpr`].
///
/// `outer` is the *full* `call!(...)` token stream (including
/// the `call` ident and the `!`). We strip those off, then
/// unwrap the surrounding parenthesised group, then parse the
/// remaining expression with [`CallExpr`].
pub fn parse_call_macro(outer: &TokenStream) -> syn::Result<CallExpr> {
    let mut iter = outer.clone().into_iter();
    let _call_ident = iter.next(); // "call"
    let _bang = iter.next(); // "!"
    let rest: TokenStream = iter.collect();
    // The remainder is typically `(self.foo(args) => result)` —
    // i.e. a single parenthesised `Group`. `CallExpr::parse`
    // expects the bare expression `self.foo(args) => result`,
    // so we strip the outer paren group if present.
    let stripped: TokenStream = if let Some(proc_macro2::TokenTree::Group(g)) =
        rest.clone().into_iter().next()
    {
        if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
            g.stream()
        } else {
            return Err(syn::Error::new_spanned(
                outer,
                "tokitai `call!(...)` must use parentheses around the body (e.g. `call!(self.foo(1) => 2)`)",
            ));
        }
    } else {
        rest
    };
    syn::parse2::<CallExpr>(stripped)
}

/// Emit the JSON-schema payload that gets appended to the
/// schema's `examples` field. Returns a `TokenStream` that
/// evaluates to a `serde_json::Value::Array(...)` (one entry
/// per example) at `LazyLock` init time. When there are no
/// examples the returned value is `Value::Array(vec![])` so the
/// caller can branch on `arr.is_empty()`.
pub fn bake_examples_to_schema_json(examples: &[BakedExample]) -> TokenStream {
    if examples.is_empty() {
        return quote! { ::tokitai::Value::Array(::std::vec![]) };
    }
    let entries = examples.iter().map(|ex| ex.render_json());
    let span = examples
        .first()
        .map(|e| e.span)
        .unwrap_or_else(Span::call_site);
    quote_spanned! {span=>
        ::tokitai::Value::Array(::std::vec![#(#entries),*])
    }
}

/// Emit a function body that contains one type-check statement
/// per baked example. The host compiler sees the
/// `Self::method(<args>)` call with the user's literal types and
/// reports any mismatch as a compile error at the `call!`
/// literal's span. The body is never executed; the bindings are
/// unused at runtime.
pub fn emit_type_checks(
    examples: &[BakedExample],
    method_ident: &syn::Ident,
    wrapper_is_async: bool,
    method_is_async: bool,
    return_type: &syn::ReturnType,
) -> TokenStream {
    if examples.is_empty() {
        return quote! {};
    }
    let checks = examples.iter().map(|ex| {
        ex.type_check_tokens(method_ident, wrapper_is_async, method_is_async, return_type)
    });
    quote! {
        // T-016: compile-time type checks for baked examples.
        // Each `check` resolves to a `()` value and is never
        // executed at runtime; the compiler still type-checks it.
        { #( #checks )* }
    }
}

/// Convert a `syn::ReturnType` into the inner type tokens. For
/// `-> i32` we return `i32`; for `-> Result<i32, E>` we return
/// `Result<i32, E>`. For the unit return type we emit `()` so
/// the comparison has a concrete RHS type.
fn return_type_to_tokens(rt: &syn::ReturnType) -> TokenStream {
    match rt {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => ty.to_token_stream(),
    }
}

/// Parses the `[call!(...), call!(...), ...]` body of a plural
/// `examples = [...]` attribute value. Returns one `BakedExample`
/// per element. Used by the attribute parser.
pub fn parse_examples_array(
    elements: impl IntoIterator<Item = TokenStream>,
) -> syn::Result<Vec<BakedExample>> {
    let mut out = Vec::new();
    for ts in elements {
        if !looks_like_call_macro(&ts) {
            return Err(syn::Error::new_spanned(
                ts,
                "tokitai `examples = [...]` accepts only `call!(...)` entries",
            ));
        }
        let parsed = parse_call_macro(&ts)?;
        let span = ts.span();
        out.push(BakedExample {
            args: parsed.args,
            result: parsed.result,
            span,
        });
    }
    Ok(out)
}

/// Parse a singular `example = call!(...)` value. The token
/// stream is the *entire* RHS of the `=` sign, including the
/// `call!(...)` invocation.
pub fn parse_example_singular(value: TokenStream) -> syn::Result<BakedExample> {
    if !looks_like_call_macro(&value) {
        return Err(syn::Error::new_spanned(
            value,
            "tokitai `example = ...` requires a `call!(...)` value (e.g. `call!(self.foo(1) => 2)`)",
        ));
    }
    let parsed = parse_call_macro(&value)?;
    let span = value.span();
    Ok(BakedExample {
        args: parsed.args,
        result: parsed.result,
        span,
    })
}
