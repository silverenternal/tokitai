//! call_tool 分发器生成
//!
//! 包含 generate_call_tool_method 函数

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::Ident;

use crate::tool::types::tool_method::ToolMethodInfo;

/// 生成 call_tool 分发方法
pub fn generate_call_tool_method(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
    let mut methods = Vec::new();
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
            let wrapper_name = format_ident!("__call_{}", method_name);

            let mut arms = Vec::new();

            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    self.#wrapper_name(args).await
                }
            });

            for alias_name in &tool.alias {
                arms.push(quote! {
                    #alias_name => {
                        self.#wrapper_name(args).await
                    }
                });
            }

            arms
        });

        methods.push(quote! {
            /// 调用工具方法
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
                    _ => Err(::tokitai::ToolError::not_found("未知工具")),
                }
            }
        });
    } else {
        let match_arms = tools.iter().flat_map(|tool| {
            let method_name = Ident::new(&tool.name, Span::call_site());
            let wrapper_name = format_ident!("__call_{}", method_name);

            let mut arms = Vec::new();

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
            /// 调用工具方法
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
                    _ => Err(::tokitai::ToolError::not_found("未知工具")),
                }
            }
        });
    }

    if has_async {
        let match_arms = tools.iter().flat_map(|tool| {
            let method_name = Ident::new(&tool.name, Span::call_site());
            let wrapper_name_sync = format_ident!("__call_{}_sync", method_name);

            let mut arms = Vec::new();

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
                    _ => Err(::tokitai::ToolError::not_found("未知工具")),
                }
            }
        });
    }

    methods
}
