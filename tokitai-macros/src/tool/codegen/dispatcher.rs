//! call_tool 分发器生成
//!
//! 包含 generate_call_tool_method 函数

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::Ident;

use crate::tool::types::tool_method::ToolMethodInfo;

/// Generate the T-013 removal guard. Returns a `TokenStream2` that, when
/// `Some`, wraps a tool's match arm to early-return
/// `ToolError::Removed` when the configured current version is at or
/// past `remove_in`. When `remove_in` is `None` no guard is emitted
/// (i.e. the call is unconditional).
///
/// The guard is keyed off the static `__TOOL_DEF_<NAME>` const the
/// macro already emits; that const is a `LazyLock<ToolDefinition>`
/// carrying the `remove_in` field, so the comparison runs at zero
/// allocation cost per call after the first.
fn generate_remove_in_guard(tool: &ToolMethodInfo) -> Option<TokenStream2> {
    let remove_in = tool.remove_in.as_deref()?;
    if remove_in.is_empty() {
        return None;
    }
    let const_name = format_ident!("__TOOL_DEF_{}", tool.name.to_uppercase());
    Some(quote! {
        // T-013: refuse to call the tool if `remove_in` is at or
        // before the program's current version. `current_version()`
        // returns `None` when the program never set one — in that
        // case the call proceeds (the deprecation is informational).
        if let Some(cur) = ::tokitai_core::current_version() {
            let cur_str: &str = cur.as_str();
            let def: &'static ::tokitai_core::ToolDefinition = Self::#const_name();
            if def.is_removed(Some(cur_str)) {
                return Err(def.removed_error(Some(cur_str)));
            }
        }
    })
}

/// Build a list of (name, replaced_by) pairs derived from this
/// impl's `#[tool]` attributes. Includes both active methods and
/// methods that were skipped / hidden, so a removed tool's
/// `replaced_by` redirect still fires through the dispatcher's
/// fallback arm. Used by the dispatcher's `_ => ...` arm to honour
/// the `replaced_by` redirect when the caller names a tool that
/// does not exist in the registry.
fn build_replaced_by_lookup(redirects: &[(String, String)]) -> Vec<TokenStream2> {
    let mut entries = Vec::new();
    for (from, to) in redirects {
        if to.is_empty() {
            continue;
        }
        let from_lit = syn::LitStr::new(from, Span::call_site());
        let to_lit = syn::LitStr::new(to, Span::call_site());
        entries.push(quote! { (#from_lit, #to_lit) });
    }
    entries
}

/// Generate the T-013 `replaced_by` redirect that fires when the
/// caller names a tool that does not exist. If the name matches
/// `replaced_by` for any removed/renamed tool, the dispatcher
/// re-invokes `call_tool` (or `call_tool_sync`) with the
/// replacement name; otherwise it returns `ToolError::NotFound`.
fn generate_replaced_by_redirect(redirects: &[(String, String)], is_async: bool) -> TokenStream2 {
    let entries = build_replaced_by_lookup(redirects);
    if entries.is_empty() {
        return quote! {
            _ => Err(::tokitai::ToolError::not_found("unknown tool"))
        };
    }
    if is_async {
        quote! {
            other => {
                // T-013: when the caller names a tool that does not
                // exist, scan the registered `replaced_by` table and
                // re-invoke ourselves with the replacement name when
                // a match is found. This keeps deprecated aliases
                // working through the dispatcher without forcing
                // every caller to do its own routing.
                const REPLACED: &[(&str, &str)] = &[#(#entries),*];
                if let Some((_, replacement)) = REPLACED.iter().find(|(from, _)| *from == other) {
                    // Disambiguate from the in-impl `call_tool`
                    // method by routing through the `ToolCaller`
                    // trait method (which is exactly the entry
                    // point callers hit from outside the impl).
                    return <Self as ::tokitai_core::ToolCaller>::call_tool(self, replacement, args).await;
                }
                Err(::tokitai::ToolError::not_found("unknown tool"))
            }
        }
    } else {
        quote! {
            other => {
                const REPLACED: &[(&str, &str)] = &[#(#entries),*];
                if let Some((_, replacement)) = REPLACED.iter().find(|(from, _)| *from == other) {
                    return <Self as ::tokitai_core::ToolCaller>::call_tool(self, replacement, args);
                }
                Err(::tokitai::ToolError::not_found("unknown tool"))
            }
        }
    }
}

/// Generate the `call_tool` / `call_tool_sync` dispatch methods.
/// `tools` is the list of active (non-skipped) tool methods;
/// `redirects` is the full `replaced_by` table (active + skipped)
/// used by the fallback arm to route old tool names to their
/// successor.
pub fn generate_call_tool_method(
    tools: &[ToolMethodInfo],
    redirects: &[(String, String)],
) -> Vec<TokenStream2> {
    // `call_tool` is always emitted; `call_tool_sync` is emitted only
    // for async impls. So the upper bound is 2.
    let mut methods = Vec::with_capacity(2);
    let has_async = tools.iter().any(|t| t.is_async);

    let tool_docs = tools
        .iter()
        .map(|tool| {
            let doc_line = format!(" - `{}`: {}", tool.tool_name, tool.description);
            quote! { #[doc = #doc_line] }
        })
        .collect::<Vec<_>>();

    if has_async {
        let match_arms = tools.iter().flat_map(|tool| {
            let method_name = Ident::new(&tool.name, Span::call_site());
            // For async methods: dispatch to the async `__call_<name>` wrapper and
            // `.await` it directly. For sync methods: dispatch to `__call_<name>_sync`
            // (the only wrapper emitted for sync methods in a mixed impl) and wrap
            // the call in `async { ... }.await` so the body is uniform.
            //
            // We MUST NOT route async methods through the `_sync` wrapper here, because
            // that wrapper uses `Handle::block_on` and would panic with
            // "Cannot start a runtime from within a runtime" when this async
            // `call_tool` is itself invoked from inside a tokio runtime (e.g. from a
            // `#[tokio::main]` or `#[tokio::test]`).
            let call_expr = if tool.is_async {
                let wrapper_name = format_ident!("__call_{}", method_name);
                quote! { self.#wrapper_name(args).await }
            } else {
                let wrapper_name_sync = format_ident!("__call_{}_sync", method_name);
                quote! { async { self.#wrapper_name_sync(args) }.await }
            };

            // T-013: optional removal guard (no-op when `remove_in`
            // is not set on the tool).
            let guard = generate_remove_in_guard(tool);

            // One arm per primary tool name + one per alias.
            let mut arms = Vec::with_capacity(1 + tool.alias.len());

            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    #guard
                    #call_expr
                }
            });

            for alias_name in &tool.alias {
                arms.push(quote! {
                    #alias_name => {
                        #guard
                        #call_expr
                    }
                });
            }

            arms
        });

        let fallback_arm = generate_replaced_by_redirect(redirects, true);

        methods.push(quote! {
            /// Invokes a tool method.
            ///
            /// # Available Tools
            #(#tool_docs)*
            pub async fn call_tool(
                &self,
                name: &str,
                args: &serde_json::Value,
            ) -> Result<serde_json::Value, ::tokitai::ToolError> {
                match name {
                    #(#match_arms)*
                    #fallback_arm
                }
            }
        });
    } else {
        let match_arms = tools.iter().flat_map(|tool| {
            let method_name = Ident::new(&tool.name, Span::call_site());
            let wrapper_name = format_ident!("__call_{}", method_name);
            let guard = generate_remove_in_guard(tool);

            // One arm per primary tool name + one per alias.
            let mut arms = Vec::with_capacity(1 + tool.alias.len());

            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    #guard
                    self.#wrapper_name(args)
                }
            });

            for alias_name in &tool.alias {
                arms.push(quote! {
                    #alias_name => {
                        #guard
                        self.#wrapper_name(args)
                    }
                });
            }

            arms
        });

        let fallback_arm = generate_replaced_by_redirect(redirects, false);

        methods.push(quote! {
            /// Invokes a tool method.
            ///
            /// # Available Tools
            #(#tool_docs)*
            pub fn call_tool(
                &self,
                name: &str,
                args: &serde_json::Value,
            ) -> Result<serde_json::Value, ::tokitai::ToolError> {
                match name {
                    #(#match_arms)*
                    #fallback_arm
                }
            }
        });
    }

    if has_async {
        let match_arms = tools.iter().flat_map(|tool| {
            let method_name = Ident::new(&tool.name, Span::call_site());
            let wrapper_name_sync = format_ident!("__call_{}_sync", method_name);
            let guard = generate_remove_in_guard(tool);

            // One arm per primary tool name + one per alias.
            let mut arms = Vec::with_capacity(1 + tool.alias.len());

            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    #guard
                    self.#wrapper_name_sync(args)
                }
            });

            for alias_name in &tool.alias {
                arms.push(quote! {
                    #alias_name => {
                        #guard
                        self.#wrapper_name_sync(args)
                    }
                });
            }

            arms
        });

        let fallback_arm = generate_replaced_by_redirect(redirects, false);

        methods.push(quote! {
            pub fn call_tool_sync(
                &self,
                name: &str,
                args: &serde_json::Value,
            ) -> Result<serde_json::Value, ::tokitai::ToolError> {
                match name {
                    #(#match_arms)*
                    #fallback_arm
                }
            }
        });
    }

    methods
}
