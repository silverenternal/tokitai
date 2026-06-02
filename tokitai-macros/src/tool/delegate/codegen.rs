//! Code generation for `#[delegate(...)]`.
//!
//! Given a parsed method signature (a `TraitItemFn`, which permits
//! signature-only items ending in `;`) and a parsed `to = "..."`
//! expression, this module produces three kinds of items:
//!
//! 1. The forwarded method itself (an `ImplItemFn` with a generated body
//!    that calls `<to_expr>.<method_name>(<args>)`).
//! 2. The `__TOOL_DEF_<NAME>` function — reused from
//!    `crate::tool::codegen::definitions::generate_tool_def_consts`.
//! 3. The `__call_<NAME>` and (for `async` methods) `__call_<NAME>_sync`
//!    wrappers — reused from
//!    `crate::tool::codegen::wrappers::generate_helper_methods`.
//!
//! We deliberately do NOT emit a `call_tool` dispatcher arm. The macro is
//! designed to be combined manually (or with a future `#[tool]` integration
//! that knows about delegate methods), so the user has the final say on
//! dispatch order.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Expr;

use crate::tool::codegen::definitions::generate_tool_def_consts;
use crate::tool::codegen::wrappers::generate_helper_methods;
use crate::tool::delegate::extract::MethodSig;
use crate::tool::types::tool_method::ToolMethodInfo;

/// The full set of items emitted by `#[delegate(...)]`.
pub struct DelegateOutput {
    /// The forwarded method (with a body), as a `TokenStream2` so that
    /// we can re-emit it in the impl-block context (or, in the
    /// standalone case, directly into the surrounding file).
    pub forwarded_method: TokenStream2,
    /// The `__TOOL_DEF_<NAME>` function, as a stream of items.
    pub tool_def_items: Vec<TokenStream2>,
    /// The `__call_<NAME>` (and optional `__call_<NAME>_sync`) wrappers, as
    /// a stream of items.
    pub wrapper_items: Vec<TokenStream2>,
}

impl DelegateOutput {
    /// Flatten all generated items into a single `TokenStream2` for
    /// `TokenStream` return.
    pub fn into_token_stream(self) -> TokenStream2 {
        let forwarded_method = &self.forwarded_method;
        let tool_def_items = &self.tool_def_items;
        let wrapper_items = &self.wrapper_items;
        quote! {
            #forwarded_method
            #(#tool_def_items)*
            #(#wrapper_items)*
        }
    }
}

/// Top-level entry point: produce the three groups of items for a single
/// `#[delegate(to = "...")]` invocation.
pub fn generate(
    method: &MethodSig,
    to_expr: &Expr,
    tool_info: &ToolMethodInfo,
) -> syn::Result<DelegateOutput> {
    let forwarded_method = build_forwarded_method(method, to_expr)?;

    // Methods without a `self` receiver (associated functions) cannot be
    // wrapped by the `#[tool]`-style `__call_*` helpers, which always
    // invoke `self.<method_name>(...)`. We therefore skip both the
    // `__TOOL_DEF_*` and `__call_*` generation for associated functions:
    // the user is expected to call the forwarded function directly via
    // its associated-function syntax (`Type::method(...)`).
    let has_receiver = method
        .sig
        .inputs
        .iter()
        .any(|arg| matches!(arg, syn::FnArg::Receiver(_)));

    // `generate_tool_def_consts` and `generate_helper_methods` take a
    // `&[ToolMethodInfo]`. They only read the slice, so we can pass a
    // single-element slice referencing the borrow. (We can't move /
    // `clone()` because `ToolMethodInfo` deliberately doesn't derive
    // `Clone`.)
    let (tool_def_items, wrapper_items) = if has_receiver {
        let tools_slice: &[ToolMethodInfo] = std::slice::from_ref(tool_info);
        let tool_def_items = generate_tool_def_consts(tools_slice);
        let wrapper_items = generate_helper_methods(tools_slice);
        (tool_def_items, wrapper_items)
    } else {
        (Vec::new(), Vec::new())
    };

    Ok(DelegateOutput {
        forwarded_method,
        tool_def_items,
        wrapper_items,
    })
}

/// Build a forwarded method body and return it as a `TokenStream2` ready to
/// splice into an impl block.
///
/// The body is constructed in one of two ways, depending on whether the
/// user-supplied method has a `self` receiver:
///
/// * **Instance method (has `self`)**: the body is
///   `<to_expr>.<method_name>(<args>)` (with `.await` for `async fn`).
///   This is the common case: a wrapper struct forwards to a method on
///   one of its fields.
///
/// * **Associated function (no `self`)**: the body is just `<to_expr>`.
///   Appending `.<method_name>(<args>)` would recurse infinitely (the
///   generated `method_name` *is* the macro-generated method), so we
///   instead treat the user's `to = "..."` as the entire body of the
///   associated function. The user is expected to write a fully-qualified
///   path expression (e.g. `to = "Config::default()"`).
fn build_forwarded_method(method: &MethodSig, to_expr: &Expr) -> syn::Result<TokenStream2> {
    let method_ident = &method.sig.ident;
    let is_async = method.sig.asyncness.is_some();
    let has_receiver = method
        .sig
        .inputs
        .iter()
        .any(|arg| matches!(arg, syn::FnArg::Receiver(_)));

    // Collect the names of all non-receiver parameters. We need them to
    // build the forwarded call (only used for the instance-method path).
    let param_idents: Vec<syn::Ident> = method
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => {
                if let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                    Some(pat_ident.ident.clone())
                } else {
                    None
                }
            }
            syn::FnArg::Receiver(_) => None,
        })
        .collect();

    // We splice the to_expr in via `quote!` so that the user's expression
    // is rendered verbatim and preserves its original span info.
    let to_expr_tokens = quote! { #to_expr };
    let body_expr = if has_receiver {
        // Instance method: `<to_expr>.<method_name>(<args>)`.
        let call_expr =
            quote! { #to_expr_tokens . #method_ident ( #(#param_idents),* ) };
        if is_async {
            quote! { #call_expr . await }
        } else {
            call_expr
        }
    } else {
        // Associated function: body is the user's `to` expression verbatim.
        // We do NOT append `.<method_name>(<args>)` because that would
        // recurse into the very method we are defining.
        to_expr_tokens
    };

    // We have to preserve *everything* about the original method signature
    // (qualifiers, generics, where-clause, return type, attrs, receiver
    // shape). We always emit `pub` (the only sensible visibility for a
    // tool) and skip the user's `pub` if they wrote one, since
    // `Signature` doesn't carry visibility.
    let attrs = &method.attrs;
    let sig = &method.sig;

    Ok(quote! {
        #(#attrs)*
        pub #sig { #body_expr }
    })
}
