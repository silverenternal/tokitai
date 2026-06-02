//! `#[delegate(to = "expr")]` proc-macro entry point.
//!
//! This is a per-method attribute that turns an inner method (one defined
//! on a field, on a sub-object, or on a fully-qualified path) into a
//! tool-eligible method WITHOUT forcing the user to write a forwarding
//! body by hand.
//!
//! ## Generated items
//!
//! For a method `pub fn foo(&self, x: u32) -> u32;` annotated with
//! `#[delegate(to = "self.inner")]` we emit:
//!
//! 1. The forwarded method, with the same signature, and a generated body
//!    that evaluates to `<to>.<method_name>(<args>)` (or `.await` for
//!    `async fn`).
//! 2. A `__TOOL_DEF_<NAME>` function (same shape as `#[tool]`).
//! 3. A `__call_<NAME>` wrapper (and `__call_<NAME>_sync` for `async`),
//!    same shape as `#[tool]`.
//!
//! We deliberately do NOT emit a `call_tool` dispatcher. The macro is
//! intended to be used either standalone (and the user wires the
//! dispatcher by hand) or in conjunction with a future `#[tool]`
//! integration that knows about delegate methods. Emitting a dispatcher
//! unconditionally would clash with `#[tool]`'s own dispatcher.
//!
//! ## Examples
//!
//! ```rust,ignore
//! pub struct OpenAIClient { inner: OpenAISdk }
//!
//! impl OpenAIClient {
//!     #[delegate(to = "self.inner")]
//!     pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, OpenAIError>;
//! }
//! ```
//!
//! After expansion, `OpenAIClient::chat` is a regular method that calls
//! `self.inner.chat(req).await` and `OpenAIClient::__TOOL_DEF_CHAT` plus
//! `OpenAIClient::__call_chat` are available for the user to wire into a
//! `call_tool` dispatcher.

use proc_macro2::TokenStream as TokenStream2;

pub mod codegen;
pub mod extract;

use extract::{build_tool_method_info, parse_to_expr, DelegateArgs};

/// Entry point invoked by the `delegate` proc-macro attribute defined in
/// `tokitai-macros/src/lib.rs`.
///
/// `args` is the contents of the attribute parens (e.g. `to = "self.inner"`).
/// `input` is the method signature the user wrote (a `pub fn ...;` token
/// stream with no body, optionally preceded by attributes like `async`).
pub fn expand(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let delegate_args = match syn::parse2::<DelegateArgs>(args) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    let method = match parse_method_sig(input) {
        Ok(m) => m,
        Err(e) => return e.to_compile_error(),
    };

    let to_expr = match parse_to_expr(&delegate_args.to_expr_str) {
        Ok(e) => e,
        Err(e) => return e.to_compile_error(),
    };

    let tool_info = match build_tool_method_info(&method) {
        Ok(info) => info,
        Err(e) => return e.to_compile_error(),
    };

    let output = match codegen::generate(&method, &to_expr, &tool_info) {
        Ok(o) => o,
        Err(e) => return e.to_compile_error(),
    };

    output.into_token_stream()
}

/// Parse the user's method-signature tokens. The user writes something
/// like:
///
/// ```text
/// #[doc = "..."]
/// pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, OpenAIError>;
/// ```
///
/// We want to end up with the leading `#[..]` attributes plus a
/// `syn::Signature`.
///
/// `TraitItemFn` is the natural fit (it allows `;`-terminated
/// signatures) but it forbids the `pub` keyword, which the user is
/// allowed to write. We therefore strip the leading `pub` (if any)
/// before parsing as `TraitItemFn`, and re-add it on the way out.
fn parse_method_sig(input: TokenStream2) -> syn::Result<extract::MethodSig> {
    // Strip a leading `pub` token if present. We scan the input as
    // a flat token stream and drop the first `pub` we see (after any
    // leading attributes). `TraitItemFn` does not accept a `pub`
    // keyword on its methods; without this strip, parsing fails with
    // "expected `fn`".
    let stripped = strip_leading_pub(input);
    let trait_fn: syn::TraitItemFn = syn::parse2(stripped)?;

    // Reject the case where the user wrote a method body. `TraitItemFn`
    // represents a default method (with a body) as having `.default =
    // Some(Block)`; a signature-only method has `default = None`.
    if trait_fn.default.is_some() {
        return Err(syn::Error::new_spanned(
            &trait_fn.sig.ident,
            "#[delegate] is meant to be applied to a method signature (no body); \
             remove the existing method body",
        ));
    }

    Ok(extract::MethodSig {
        attrs: trait_fn.attrs,
        sig: trait_fn.sig,
    })
}

/// Walk the input token stream, skipping past any leading
/// `#[...]` attributes. The first `pub` ident we encounter after
/// the leading-attribute run is dropped (it is not part of a
/// `TraitItemFn`). Everything else is returned unchanged.
fn strip_leading_pub(input: TokenStream2) -> TokenStream2 {
    let mut out = TokenStream2::new();
    let mut iter = input.into_iter().peekable();
    let mut past_attrs = false;
    let mut pub_dropped = false;

    while let Some(tt) = iter.next() {
        match &tt {
            proc_macro2::TokenTree::Punct(p) if p.as_char() == '#' => {
                // Start of an attribute. Push the `#`, then collect the
                // rest of the attribute as-is (syn parses the whole
                // `#[...]` group as one token stream).
                out.extend(std::iter::once(tt));
                // Collect tokens until the matching `]`.
                let mut depth: i32 = 0;
                for next in iter.by_ref() {
                    match &next {
                        proc_macro2::TokenTree::Punct(p) if p.as_char() == '[' => {
                            depth += 1;
                        }
                        proc_macro2::TokenTree::Punct(p) if p.as_char() == ']' => {
                            depth -= 1;
                        }
                        _ => {}
                    }
                    out.extend(std::iter::once(next));
                    if depth == 0 {
                        break;
                    }
                }
            }
            proc_macro2::TokenTree::Ident(i) if !past_attrs && !pub_dropped && i == "pub" => {
                // Drop this `pub` ident. Mark the flag so we only drop
                // the first one.
                pub_dropped = true;
                past_attrs = true;
            }
            _ => {
                past_attrs = true;
                out.extend(std::iter::once(tt));
            }
        }
    }
    out
}
