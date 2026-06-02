//! `#[wrap]` proc-macro implementation.
//!
//! `#[wrap]` is a natural extension of `#[tool]`: instead of registering
//! every public method on an impl block, it lets the user pre-select a
//! curated subset of methods and reuses the same compile-time pipeline
//! to generate `__TOOL_DEF_*`, `__call_*`, `call_tool`, and the
//! `ToolProvider`/`ToolCaller` impls.
//!
//! In addition, `#[wrap]` generates a `new(client: T) -> Self`
//! constructor that wires the inner client into the wrapper struct,
//! making it ergonomic to expose a third-party API client as a set of
//! AI tools.
//!
//! ## Example
//!
//! ```rust,ignore
//! pub struct GitHubClient { pub client: InnerClient }
//!
//! #[wrap(client = InnerClient, methods = [get_user, list_repos])]
//! impl GitHubClient {
//!     pub fn get_user(&self, login: String) -> Result<User, String> {
//!         // user-written body that calls self.client.fetch(...)
//!     }
//! }
//! ```

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse2, ItemImpl};

mod codegen;
mod extract;
mod inject;

/// Parsed form of `#[wrap(client = TYPE, methods = [a, b, c])]`.
pub(crate) struct WrapArgs {
    /// The inner-client type passed in `client = TYPE`.
    pub client_ty: syn::Type,
    /// The field name on the wrapper struct (default: `client`).
    pub client_field: syn::Ident,
    /// The method names listed in `methods = [...]`.
    pub methods: Vec<syn::Ident>,
}

/// Macro entry point. Resolves errors to `compile_error!` tokens.
pub fn expand(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    // 1. Parse `client = TYPE, methods = [a, b, ...]`.
    let wrap_args = match extract::parse_wrap_args(args) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };

    // 2. Parse the impl block. Anything that is not an impl block is
    //    surfaced as a compile error so the user gets a clear message.
    let mut impl_item: ItemImpl = match parse2(input) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    // 3. Filter the impl's methods down to those listed in `methods`.
    let tool_methods = match inject::filter_tool_methods(&impl_item, &wrap_args.methods) {
        Ok(t) => t,
        Err(e) => return e.to_compile_error(),
    };

    // 4. Generate every per-impl artifact (`__TOOL_DEF_*`, `__call_*`,
    //    `call_tool`, `configure_tool`, `__TOOL_COUNT`, etc.).
    let generated_items = codegen::generate(&impl_item.self_ty, &wrap_args, &tool_methods);

    // 5. Splice the generated items into the impl block as `ImplItem`s.
    if let Err(e) = inject::append_items(&mut impl_item, &generated_items) {
        return e.to_compile_error();
    }

    // 6. Emit the impl block + the `ToolProvider` / `ToolCaller` impls.
    let impl_ty = &impl_item.self_ty;
    let tool_provider_impl = codegen::tool_provider_impl(impl_ty);
    let tool_caller_impl = codegen::tool_caller_impl(impl_ty);

    quote! {
        #impl_item

        #tool_provider_impl

        #tool_caller_impl
    }
}
