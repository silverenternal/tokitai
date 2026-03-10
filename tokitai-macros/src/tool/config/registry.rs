//! 配置宏实现
//!
//! 包含 tokitai! 配置宏的实现

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
            let method_name: Ident = content.parse()?;
            content.parse::<token::Colon>()?;

            let method_content;
            syn::braced!(method_content in content);

            let mut desc = None;
            let mut tags = None;
            let mut params = BTreeMap::new();

            while !method_content.is_empty() {
                let key: Ident = method_content.parse()?;
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
                        let params_content;
                        syn::braced!(params_content in method_content);

                        while !params_content.is_empty() {
                            let param_name: Ident = params_content.parse()?;
                            params_content.parse::<token::Colon>()?;

                            let param_content;
                            syn::braced!(param_content in params_content);

                            let mut param_desc = None;
                            let mut param_example = None;

                            while !param_content.is_empty() {
                                let param_key: Ident = param_content.parse()?;
                                param_content.parse::<token::Colon>()?;

                                match param_key.to_string().as_str() {
                                    "desc" => {
                                        let value: LitStr = param_content.parse()?;
                                        param_desc = Some(value.value());
                                    }
                                    "example" => {
                                        param_example =
                                            parse_json_value(&param_content).ok().flatten();
                                    }
                                    _ => {
                                        let _ = param_content.parse::<Expr>();
                                    }
                                }

                                if param_content.peek(token::Comma) {
                                    param_content.parse::<token::Comma>()?;
                                }
                            }

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
                        let _ = method_content.parse::<Expr>();
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
                ::tokitai::ToolConfig::Desc(#desc.to_string())
            });
        }

        if let Some(ref tags) = method.tags {
            config_items.push(quote! {
                ::tokitai::ToolConfig::Tags(vec![#(#tags.to_string()),*])
            });
        }

        for (param_name, param_config) in &method.params {
            if let Some(ref param_desc) = param_config.desc {
                config_items.push(quote! {
                    ::tokitai::ToolConfig::ParamDesc {
                        name: #param_name.to_string(),
                        desc: #param_desc.to_string()
                    }
                });
            }
            if let Some(ref param_example) = param_config.example {
                let example_json = serde_json::to_string(param_example).unwrap_or_default();
                config_items.push(quote! {
                    ::tokitai::ToolConfig::ParamExample {
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
