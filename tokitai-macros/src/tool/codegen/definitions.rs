//! 工具定义生成
//!
//! 包含 generate_tool_def_consts、generate_all_tool_defs_array 等函数

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::Type;

use crate::tool::schema::gen::SchemaGenConfig;
use crate::tool::types::tool_method::ToolMethodInfo;

/// 生成编译期工具定义函数
pub fn generate_tool_def_consts(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
    let mut consts = Vec::new();

    for tool in tools {
        if tool.is_generic {
            let name = &tool.name;
            consts.push(quote! {
                compile_error!(concat!(
                    "🔧 工具方法 `",
                    #name,
                    "` 使用了泛型参数，这不被支持。\n",
                    "💡 解决方案：\n",
                    "   1. 使用具体类型：fn ",
                    #name,
                    "(data: MyType) -> String\n",
                    "   2. 使用 serde_json::Value: fn ",
                    #name,
                    "(data: Value) -> String\n",
                    "   3. 在方法内部手动反序列化"
                ));
            });
            continue;
        }

        let const_name = format_ident!("__TOOL_DEF_{}", tool.name.to_uppercase());
        let tool_name = &tool.tool_name;
        let description = &tool.description;

        let schema_json = crate::tool::schema::gen::generate_schema_json_with_deprecated_and_tags(
            &SchemaGenConfig::new(&tool.params)
                .deprecated(tool.deprecated)
                .replaced_by(tool.replaced_by.as_deref())
                .context(tool.context.as_deref())
                .tags(&tool.tags)
                .return_description(tool.return_description.as_deref())
                .example_input(tool.example_input.as_ref())
                .param_order(tool.param_order.as_deref())
                .example_output(tool.example_output.as_deref())
                .deprecated_note(tool.deprecated_note.as_deref())
                .deprecated_since(tool.deprecated_since.as_deref())
                .remove_in(tool.remove_in.as_deref())
                .group(tool.group.as_deref())
                .cache(tool.cache.as_deref())
                .rate_limit(tool.rate_limit.as_deref()),
        );

        let version_tokens = tool
            .version
            .as_ref()
            .map(|v| {
                quote! { .with_version(#v) }
            })
            .unwrap_or_else(|| quote! {});

        let deprecated_tokens = if tool.deprecated_since.is_some()
            || tool.remove_in.is_some()
            || tool.replaced_by.is_some()
        {
            let dep_since = tool.deprecated_since.as_deref().unwrap_or("");
            let rem_in = tool.remove_in.as_deref().unwrap_or("");
            let repl_by = tool.replaced_by.as_deref().unwrap_or("");
            quote! { .with_deprecated(#dep_since, #rem_in, #repl_by) }
        } else {
            quote! {}
        };

        consts.push(quote! {
            fn #const_name() -> &'static ::tokitai::ToolDefinition {
                static DEF: ::std::sync::LazyLock<::tokitai::ToolDefinition> = ::std::sync::LazyLock::new(|| {
                    ::tokitai::ToolDefinition::new(#tool_name, #description, #schema_json) #version_tokens #deprecated_tokens
                });
                &*DEF
            }
        });

        for (i, alias_name) in tool.alias.iter().enumerate() {
            let alias_const_name =
                format_ident!("__TOOL_DEF_ALIAS_{}_{}", tool.name.to_uppercase(), i);
            let alias_desc = format!("(别名：{}) {}", tool.tool_name, tool.description);

            consts.push(quote! {
                fn #alias_const_name() -> &'static ::tokitai::ToolDefinition {
                    static DEF: ::std::sync::LazyLock<::tokitai::ToolDefinition> = ::std::sync::LazyLock::new(|| {
                        ::tokitai::ToolDefinition::new(#alias_name, #alias_desc, #schema_json)
                    });
                    &*DEF
                }
            });
        }
    }

    consts
}

/// 生成所有工具定义的数组
pub fn generate_all_tool_defs_array(
    tools: &[ToolMethodInfo],
    impl_type: &Type,
) -> Vec<TokenStream2> {
    let mut defs = Vec::new();

    for tool in tools {
        let const_name = format_ident!("__TOOL_DEF_{}", tool.name.to_uppercase());
        defs.push(quote! { #impl_type::#const_name() });

        for (i, _alias_name) in tool.alias.iter().enumerate() {
            let alias_const_name =
                format_ident!("__TOOL_DEF_ALIAS_{}_{}", tool.name.to_uppercase(), i);
            defs.push(quote! { #impl_type::#alias_const_name() });
        }
    }

    defs
}
