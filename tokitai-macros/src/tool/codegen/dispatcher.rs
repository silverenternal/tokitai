//! call_tool 分发器生成
//!
//! 包含 generate_call_tool_method 函数

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::Ident;

use crate::tool::types::tool_method::ToolMethodInfo;

/// 生成 call_tool 分发方法
pub fn generate_call_tool_method(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
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

            // One arm per primary tool name + one per alias.
            let mut arms = Vec::with_capacity(1 + tool.alias.len());

            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    #call_expr
                }
            });

            for alias_name in &tool.alias {
                arms.push(quote! {
                    #alias_name => {
                        #call_expr
                    }
                });
            }

            arms
        });

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
                    _ => Err(::tokitai::ToolError::not_found("unknown tool")),
                }
            }
        });
    } else {
        let match_arms = tools.iter().flat_map(|tool| {
            let method_name = Ident::new(&tool.name, Span::call_site());
            let wrapper_name = format_ident!("__call_{}", method_name);

            // One arm per primary tool name + one per alias.
            let mut arms = Vec::with_capacity(1 + tool.alias.len());

            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    self.#wrapper_name(args)
                }
            });

            for alias_name in &tool.alias {
                arms.push(quote! {
                    #alias_name => {
                        self.#wrapper_name(args)
                    }
                });
            }

            arms
        });

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
                    _ => Err(::tokitai::ToolError::not_found("unknown tool")),
                }
            }
        });
    }

    if has_async {
        let match_arms = tools.iter().flat_map(|tool| {
            let method_name = Ident::new(&tool.name, Span::call_site());
            let wrapper_name_sync = format_ident!("__call_{}_sync", method_name);

            // One arm per primary tool name + one per alias.
            let mut arms = Vec::with_capacity(1 + tool.alias.len());

            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    self.#wrapper_name_sync(args)
                }
            });

            for alias_name in &tool.alias {
                arms.push(quote! {
                    #alias_name => {
                        self.#wrapper_name_sync(args)
                    }
                });
            }

            arms
        });

        methods.push(quote! {
            pub fn call_tool_sync(
                &self,
                name: &str,
                args: &serde_json::Value,
            ) -> Result<serde_json::Value, ::tokitai::ToolError> {
                match name {
                    #(#match_arms)*
                    _ => Err(::tokitai::ToolError::not_found("unknown tool")),
                }
            }
        });
    }

    methods
}
