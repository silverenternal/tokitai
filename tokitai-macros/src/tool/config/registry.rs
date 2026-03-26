//! 配置宏实现
//!
//! 包含 tokitai! 配置宏的实现
//!
//! # 语法
//!
//! ## 新语法（扁平化，推荐）
//! ```rust,ignore
//! tokitai! {
//!     MyStruct {
//!         method_name {
//!             desc: "描述",
//!             tags: ["tag1", "tag2"],
//!             param_name: "参数描述" | { desc: "描述", example: "值" },
//!         }
//!     }
//! }
//! ```
//!
//! ## 旧语法（嵌套，向后兼容）
//! ```rust,ignore
//! tokitai! {
//!     MyStruct {
//!         method_name: {
//!             desc: "描述",
//!             params: {
//!                 param_name: {
//!                     desc: "描述",
//!                     example: "值"
//!                 }
//!             }
//!         }
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, token, Expr, Ident, LitStr,
};

/// 配置宏输入结构
struct ConfigInput {
    struct_name: Ident,
    methods: Vec<MethodConfig>,
}

struct MethodConfig {
    method_name: Ident,
    desc: Option<String>,
    tags: Option<Vec<String>>,
    params: BTreeMap<String, ParamConfig>,
}

struct ParamConfig {
    desc: Option<String>,
    example: Option<serde_json::Value>,
}

impl Parse for ConfigInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let struct_name: Ident = input.parse()?;

        let content;
        syn::braced!(content in input);

        let mut methods = Vec::new();

        while !content.is_empty() {
            // 解析方法名
            let method_name: Ident = content.parse()?;

            // 支持两种语法：
            // 1. 新语法：method_name { ... } （无冒号）
            // 2. 旧语法：method_name: { ... } （有冒号）
            if content.peek(token::Colon) {
                content.parse::<token::Colon>()?;
            }

            let method_content;
            syn::braced!(method_content in content);

            let mut desc = None;
            let mut tags = None;
            let mut params = BTreeMap::new();

            while !method_content.is_empty() {
                let key: Ident = method_content.parse()?;

                // 检查是新语法还是旧语法
                let is_param_desc = method_content.peek(token::Colon);

                if is_param_desc {
                    // 旧语法或参数描述
                    method_content.parse::<token::Colon>()?;

                    match key.to_string().as_str() {
                        "desc" => {
                            let value: LitStr = method_content.parse()?;
                            desc = Some(value.value());
                        }
                        "tags" => {
                            let tag_content;
                            syn::bracketed!(tag_content in method_content);
                            let mut tag_list = Vec::new();
                            while !tag_content.is_empty() {
                                let tag: LitStr = tag_content.parse()?;
                                tag_list.push(tag.value());
                                if tag_content.peek(token::Comma) {
                                    tag_content.parse::<token::Comma>()?;
                                }
                            }
                            tags = Some(tag_list);
                        }
                        "params" => {
                            // 旧语法：params: { param_name: { ... } }
                            let params_content;
                            syn::braced!(params_content in method_content);

                            while !params_content.is_empty() {
                                let param_name: Ident = params_content.parse()?;
                                params_content.parse::<token::Colon>()?;

                                let param_content;
                                syn::braced!(param_content in params_content);

                                let (param_desc, param_example) =
                                    parse_param_config(&param_content)?;

                                params.insert(
                                    param_name.to_string(),
                                    ParamConfig {
                                        desc: param_desc,
                                        example: param_example,
                                    },
                                );

                                if params_content.peek(token::Comma) {
                                    params_content.parse::<token::Comma>()?;
                                }
                            }
                        }
                        _ => {
                            // 未知字段，跳过
                            let _ = method_content.parse::<Expr>();
                        }
                    }
                } else {
                    // 新语法：param_name: "desc" 或 param_name { ... }
                    let key_str = key.to_string();

                    // 检查是否是特殊字段
                    if key_str == "desc" {
                        method_content.parse::<token::Colon>()?;
                        let value: LitStr = method_content.parse()?;
                        desc = Some(value.value());
                    } else if key_str == "tags" {
                        method_content.parse::<token::Colon>()?;
                        let tag_content;
                        syn::bracketed!(tag_content in method_content);
                        let mut tag_list = Vec::new();
                        while !tag_content.is_empty() {
                            let tag: LitStr = tag_content.parse()?;
                            tag_list.push(tag.value());
                            if tag_content.peek(token::Comma) {
                                tag_content.parse::<token::Comma>()?;
                            }
                        }
                        tags = Some(tag_list);
                    } else {
                        // 参数定义
                        let param_name = key_str;

                        // 支持两种形式：
                        // 1. param_name: "desc"
                        // 2. param_name { desc: "desc", example: "value" }
                        if method_content.peek(token::Colon) {
                            method_content.parse::<token::Colon>()?;

                            // 检查是否是字符串字面量（简单形式）
                            if let Ok(lit_str) = method_content.parse::<LitStr>() {
                                params.insert(
                                    param_name,
                                    ParamConfig {
                                        desc: Some(lit_str.value()),
                                        example: None,
                                    },
                                );
                            } else {
                                // 复杂形式：{ desc: "desc", example: "value" }
                                let param_content;
                                syn::braced!(param_content in method_content);
                                let (param_desc, param_example) =
                                    parse_param_config(&param_content)?;
                                params.insert(
                                    param_name,
                                    ParamConfig {
                                        desc: param_desc,
                                        example: param_example,
                                    },
                                );
                            }
                        } else if method_content.peek(token::Brace) {
                            // param_name { ... } 形式
                            let param_content;
                            syn::braced!(param_content in method_content);
                            let (param_desc, param_example) = parse_param_config(&param_content)?;
                            params.insert(
                                param_name,
                                ParamConfig {
                                    desc: param_desc,
                                    example: param_example,
                                },
                            );
                        }
                    }
                }

                if method_content.peek(token::Comma) {
                    method_content.parse::<token::Comma>()?;
                }
            }

            methods.push(MethodConfig {
                method_name,
                desc,
                tags,
                params,
            });

            if content.peek(token::Comma) {
                content.parse::<token::Comma>()?;
            }
        }

        Ok(ConfigInput {
            struct_name,
            methods,
        })
    }
}

/// 解析参数配置
fn parse_param_config(
    input: ParseStream,
) -> syn::Result<(Option<String>, Option<serde_json::Value>)> {
    let mut param_desc = None;
    let mut param_example = None;

    while !input.is_empty() {
        let param_key: Ident = input.parse()?;
        input.parse::<token::Colon>()?;

        match param_key.to_string().as_str() {
            "desc" => {
                let value: LitStr = input.parse()?;
                param_desc = Some(value.value());
            }
            "example" => {
                param_example = parse_json_value(input).ok().flatten();
            }
            _ => {
                // 未知字段，跳过
                let _ = input.parse::<Expr>();
            }
        }

        if input.peek(token::Comma) {
            input.parse::<token::Comma>()?;
        }
    }

    Ok((param_desc, param_example))
}

/// 解析 JSON 值
fn parse_json_value(input: ParseStream) -> syn::Result<Option<serde_json::Value>> {
    if input.peek(syn::LitInt) {
        let lit_int: syn::LitInt = input.parse()?;
        if let Ok(val) = lit_int.base10_parse::<i64>() {
            return Ok(Some(serde_json::json!(val)));
        }
    }
    if input.peek(syn::LitFloat) {
        let lit_float: syn::LitFloat = input.parse()?;
        if let Ok(val) = lit_float.base10_parse::<f64>() {
            return Ok(Some(serde_json::json!(val)));
        }
    }
    if input.peek(syn::LitBool) {
        let lit_bool: syn::LitBool = input.parse()?;
        return Ok(Some(serde_json::json!(lit_bool.value)));
    }

    if let Ok(lit_str) = input.parse::<LitStr>() {
        let str_value = lit_str.value();
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&str_value) {
            return Ok(Some(val));
        }
        return Ok(Some(serde_json::Value::String(str_value)));
    }

    Ok(None)
}

/// 配置宏主函数
pub fn config(item: TokenStream) -> TokenStream {
    let config_input = parse_macro_input!(item as ConfigInput);

    let struct_name = &config_input.struct_name;

    let config_init_name = format_ident!("__CONFIG_INIT_{}", struct_name);

    let mut method_configs = Vec::new();

    for method in &config_input.methods {
        let method_name = &method.method_name;
        let mut config_items = Vec::new();

        if let Some(ref desc) = method.desc {
            config_items.push(quote! {
                ::tokitai_core::ToolConfig::Desc(#desc.to_string())
            });
        }

        if let Some(ref tags) = method.tags {
            config_items.push(quote! {
                ::tokitai_core::ToolConfig::Tags(vec![#(#tags.to_string()),*])
            });
        }

        for (param_name, param_config) in &method.params {
            if let Some(ref param_desc) = param_config.desc {
                config_items.push(quote! {
                    ::tokitai_core::ToolConfig::ParamDesc {
                        name: #param_name.to_string(),
                        desc: #param_desc.to_string()
                    }
                });
            }
            if let Some(ref param_example) = param_config.example {
                let example_json = serde_json::to_string(param_example).unwrap_or_default();
                config_items.push(quote! {
                    ::tokitai_core::ToolConfig::ParamExample {
                        name: #param_name.to_string(),
                        example: serde_json::json!(#example_json)
                    }
                });
            }
        }

        if !config_items.is_empty() {
            let method_name_str = method_name.to_string();
            method_configs.push(quote! {
                #struct_name::configure_tool(#method_name_str, &[#(#config_items),*]);
            });
        }
    }

    let output = quote! {
        #[used]
        static #config_init_name: ::std::sync::LazyLock<()> = ::std::sync::LazyLock::new(|| {
            #(#method_configs)*
        });
    };

    TokenStream::from(output)
}
