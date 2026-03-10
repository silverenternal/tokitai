//! 包装方法生成
//!
//! 包含 generate_wrapper_method_sync、generate_wrapper_method、generate_helper_methods 等函数

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Expr, Ident};

use crate::tool::types::tool_method::ToolMethodInfo;

/// 打印警告信息（在测试环境下抑制输出）
macro_rules! warn_if_not_test {
    ($($arg:tt)*) => {
        if !cfg!(test) {
            eprintln!($($arg)*);
        }
    };
}

/// 生成参数解析辅助方法
pub fn generate_helper_methods(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
    let mut methods = Vec::new();
    let has_async = tools.iter().any(|t| t.is_async);

    for tool in tools {
        if has_async {
            methods.push(generate_wrapper_method(tool, true));
            if tool.is_async {
                methods.push(generate_wrapper_method(tool, false));
            } else {
                methods.push(generate_wrapper_method_sync(tool));
            }
        } else {
            methods.push(generate_wrapper_method_sync(tool));
        }
    }

    methods
}

/// 生成同步包装方法
pub fn generate_wrapper_method_sync(tool: &ToolMethodInfo) -> TokenStream2 {
    let method_name = Ident::new(&tool.name, Span::call_site());
    let wrapper_name = format_ident!("__call_{}", method_name);
    let params = &tool.params;

    let param_parsing = params.iter().map(|p| {
        let param_name = &p.name;
        let schema_name_str = &p.schema_name;
        let param_type = &p.ty;

        if p.is_option {
            quote! {
                let #param_name = args.get(#schema_name_str)
                    .and_then(|v| v.as_null().map(|_| None))
                    .unwrap_or_else(|| {
                        args.get(#schema_name_str).map(|v| serde_json::from_value(v.clone()).ok())
                    })
                    .flatten();
            }
        } else {
            quote! {
                let #param_name = args.get(#schema_name_str)
                    .ok_or_else(|| ::tokitai::ToolError::validation_error(
                        format!("缺少必需参数 '{}' (类型：{})", #schema_name_str, std::any::type_name::<#param_type>())
                    ))?;
                let mut #param_name: #param_type = serde_json::from_value(#param_name.clone())
                    .map_err(|e| ::tokitai::ToolError::validation_error(
                        format!("参数类型错误：{} (期望类型：{})", #schema_name_str, std::any::type_name::<#param_type>())
                    ))?;
            }
        }
    });

    let param_validations = params.iter().flat_map(|p| {
        let mut validations = Vec::new();
        let param_name = &p.name;
        let schema_name_str = &p.schema_name;

        if let Some(validate_expr) = &p.validate {
            let validate_code = validate_expr.replace("value", &format!("{}", param_name));
            let validate_expr_tokens: Expr = match syn::parse_str(&validate_code) {
                Ok(expr) => expr,
                Err(e) => {
                    warn_if_not_test!("[tokitai] warning: failed to parse validation expression: {} - {}", validate_code, e);
                    return Vec::new();
                }
            };
            let error_msg = if let Some(ref msg_zh) = p.validate_msg_zh {
                let msg_en = p.validate_msg_en.as_deref().unwrap_or("");
                quote! {
                    if std::env::var("LANG").unwrap_or_default().starts_with("zh") ||
                       std::env::var("LC_ALL").unwrap_or_default().starts_with("zh") {
                        #msg_zh.to_string()
                    } else {
                        let msg_en_str = #msg_en;
                        if !msg_en_str.is_empty() {
                            msg_en_str.to_string()
                        } else {
                            format!("参数 '{}' 验证失败：{}", #schema_name_str, #validate_expr)
                        }
                    }
                }
            } else if let Some(ref msg) = p.validate_msg {
                quote! { #msg.to_string() }
            } else {
                quote! { format!("参数 '{}' 验证失败：{}", #schema_name_str, #validate_expr) }
            };
            validations.push(quote! {
                if !(#validate_expr_tokens) {
                    return Err(::tokitai::ToolError::validation_error(#error_msg));
                }
            });
        }

        if let Some(one_of) = &p.one_of {
            if p.is_option {
                let allowed_values = one_of;
                validations.push(quote! {
                    if let Some(ref val) = #param_name {
                        if ![#(#allowed_values),*].contains(&val.as_str()) {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 '{}' 不在允许的范围内，允许的值：{}", #schema_name_str, val, [#(#allowed_values),*].join(", "))
                            ));
                        }
                    }
                });
            } else {
                let allowed_values = one_of;
                validations.push(quote! {
                    if ![#(#allowed_values),*].contains(&#param_name.as_str()) {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 '{}' 不在允许的范围内，允许的值：{}", #schema_name_str, #param_name, [#(#allowed_values),*].join(", "))
                        ));
                    }
                });
            }
        }

        if let Some(pattern) = &p.pattern {
            if p.is_option {
                validations.push(quote! {
                    if let Some(ref val) = #param_name {
                        if !val.contains(#pattern) {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 '{}' 不包含模式：{}", #schema_name_str, val, #pattern)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    if !#param_name.contains(#pattern) {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 '{}' 不包含模式：{}", #schema_name_str, #param_name, #pattern)
                        ));
                    }
                });
            }
        }

        if let Some(min) = p.min {
            if p.is_option {
                validations.push(quote! {
                    if let Some(val) = #param_name.map(|n| n as f64) {
                        if val < #min {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 {} 小于最小值 {}", #schema_name_str, val, #min)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    if val < #min {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 {} 小于最小值 {}", #schema_name_str, val, #min)
                        ));
                    }
                });
            }
        }

        if let Some(max) = p.max {
            if p.is_option {
                validations.push(quote! {
                    if let Some(val) = #param_name.map(|n| n as f64) {
                        if val > #max {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 {} 大于最大值 {}", #schema_name_str, val, #max)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    if val > #max {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 {} 大于最大值 {}", #schema_name_str, val, #max)
                        ));
                    }
                });
            }
        }

        if let Some(min_len) = p.min_length {
            if p.is_option {
                validations.push(quote! {
                    if let Some(ref val) = #param_name {
                        let len = val.len();
                        if len < #min_len {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的长度 {} 小于最小长度 {}", #schema_name_str, len, #min_len)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let len = #param_name.len();
                    if len < #min_len {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的长度 {} 小于最小长度 {}", #schema_name_str, len, #min_len)
                        ));
                    }
                });
            }
        }

        if let Some(max_len) = p.max_length {
            if p.is_option {
                validations.push(quote! {
                    if let Some(ref val) = #param_name {
                        let len = val.len();
                        if len > #max_len {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的长度 {} 大于最大长度 {}", #schema_name_str, len, #max_len)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let len = #param_name.len();
                    if len > #max_len {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的长度 {} 大于最大长度 {}", #schema_name_str, len, #max_len)
                        ));
                    }
                });
            }
        }

        if let Some(multiple) = p.multiple_of {
            if p.is_option {
                validations.push(quote! {
                    if let Some(val) = #param_name.map(|n| n as f64) {
                        let quotient = val / #multiple;
                        let remainder = (quotient - quotient.round()).abs();
                        if remainder > 0.0001 {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 {} 不是 {} 的倍数", #schema_name_str, val, #multiple)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    let quotient = val / #multiple;
                    let remainder = (quotient - quotient.round()).abs();
                    if remainder > 0.0001 {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 {} 不是 {} 的倍数", #schema_name_str, val, #multiple)
                        ));
                    }
                });
            }
        }

        validations
    });

    let param_transforms = params.iter().filter_map(|p| {
        if let Some(transform_expr) = &p.transform {
            let param_name = &p.name;
            let transform_code = transform_expr.replace("value", &format!("{}", param_name));
            let transform_expr_tokens: Expr = match syn::parse_str(&transform_code) {
                Ok(expr) => expr,
                Err(e) => {
                    warn_if_not_test!(
                        "[tokitai] warning: failed to parse transform expression: {} - {}",
                        transform_code,
                        e
                    );
                    return None;
                }
            };
            Some(quote! {
                #param_name = #transform_expr_tokens;
            })
        } else {
            None
        }
    });

    let param_names: Vec<&Ident> = params.iter().map(|p| &p.name).collect();

    let result_handling = if tool.is_result {
        quote! {
            match result {
                Ok(v) => Ok(serde_json::to_value(v).unwrap()),
                Err(e) => Err(::tokitai::ToolError::internal_error(format!("{}", e))),
            }
        }
    } else {
        quote! {
            Ok(serde_json::to_value(result).unwrap())
        }
    };

    quote! {
        #[allow(clippy::all)]
        fn #wrapper_name(&self, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> {
            use serde_json::Value;

            #(#param_parsing)*

            // 参数验证
            #(#param_validations)*

            // 参数转换
            #(#param_transforms)*

            let result = self.#method_name(#(#param_names),*);
            #result_handling
        }
    }
}

/// 生成异步包装方法
pub fn generate_wrapper_method(tool: &ToolMethodInfo, is_async: bool) -> TokenStream2 {
    let method_name = Ident::new(&tool.name, Span::call_site());
    let wrapper_name = if is_async {
        format_ident!("__call_{}", method_name)
    } else {
        format_ident!("__call_{}_sync", method_name)
    };
    let params = &tool.params;

    let param_parsing = params.iter().map(|p| {
        let param_name = &p.name;
        let schema_name_str = &p.schema_name;
        let param_type = &p.ty;

        if p.is_option {
            quote! {
                let #param_name = args.get(#schema_name_str)
                    .and_then(|v| v.as_null().map(|_| None))
                    .unwrap_or_else(|| {
                        args.get(#schema_name_str).map(|v| serde_json::from_value(v.clone()).ok())
                    })
                    .flatten();
            }
        } else {
            quote! {
                let #param_name = args.get(#schema_name_str)
                    .ok_or_else(|| ::tokitai::ToolError::validation_error(
                        format!("缺少必需参数 '{}' (类型：{})", #schema_name_str, std::any::type_name::<#param_type>())
                    ))?;
                let mut #param_name: #param_type = serde_json::from_value(#param_name.clone())
                    .map_err(|e| ::tokitai::ToolError::validation_error(
                        format!("参数类型错误：{} (期望类型：{})", #schema_name_str, std::any::type_name::<#param_type>())
                    ))?;
            }
        }
    });

    let param_validations = params.iter().flat_map(|p| {
        let mut validations = Vec::new();
        let param_name = &p.name;
        let schema_name_str = &p.schema_name;

        if let Some(validate_expr) = &p.validate {
            let validate_code = validate_expr.replace("value", &format!("{}", param_name));
            let validate_expr_tokens: Expr = match syn::parse_str(&validate_code) {
                Ok(expr) => expr,
                Err(e) => {
                    warn_if_not_test!("[tokitai] warning: failed to parse validation expression: {} - {}", validate_code, e);
                    return Vec::new();
                }
            };
            let error_msg = if let Some(ref msg_zh) = p.validate_msg_zh {
                let msg_en = p.validate_msg_en.as_deref().unwrap_or("");
                quote! {
                    if std::env::var("LANG").unwrap_or_default().starts_with("zh") ||
                       std::env::var("LC_ALL").unwrap_or_default().starts_with("zh") {
                        #msg_zh.to_string()
                    } else {
                        let msg_en_str = #msg_en;
                        if !msg_en_str.is_empty() {
                            msg_en_str.to_string()
                        } else {
                            format!("参数 '{}' 验证失败：{}", #schema_name_str, #validate_expr)
                        }
                    }
                }
            } else if let Some(ref msg) = p.validate_msg {
                quote! { #msg.to_string() }
            } else {
                quote! { format!("参数 '{}' 验证失败：{}", #schema_name_str, #validate_expr) }
            };
            validations.push(quote! {
                if !(#validate_expr_tokens) {
                    return Err(::tokitai::ToolError::validation_error(#error_msg));
                }
            });
        }

        if let Some(one_of) = &p.one_of {
            if p.is_option {
                let allowed_values = one_of;
                validations.push(quote! {
                    if let Some(ref val) = #param_name {
                        if ![#(#allowed_values),*].contains(&val.as_str()) {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 '{}' 不在允许的范围内，允许的值：{}", #schema_name_str, val, [#(#allowed_values),*].join(", "))
                            ));
                        }
                    }
                });
            } else {
                let allowed_values = one_of;
                validations.push(quote! {
                    if ![#(#allowed_values),*].contains(&#param_name.as_str()) {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 '{}' 不在允许的范围内，允许的值：{}", #schema_name_str, #param_name, [#(#allowed_values),*].join(", "))
                        ));
                    }
                });
            }
        }

        if let Some(pattern) = &p.pattern {
            if p.is_option {
                validations.push(quote! {
                    if let Some(ref val) = #param_name {
                        if !val.contains(#pattern) {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 '{}' 不包含模式：{}", #schema_name_str, val, #pattern)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    if !#param_name.contains(#pattern) {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 '{}' 不包含模式：{}", #schema_name_str, #param_name, #pattern)
                        ));
                    }
                });
            }
        }

        if let Some(min) = p.min {
            if p.is_option {
                validations.push(quote! {
                    if let Some(val) = #param_name.map(|n| n as f64) {
                        if val < #min {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 {} 小于最小值 {}", #schema_name_str, val, #min)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    if val < #min {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 {} 小于最小值 {}", #schema_name_str, val, #min)
                        ));
                    }
                });
            }
        }

        if let Some(max) = p.max {
            if p.is_option {
                validations.push(quote! {
                    if let Some(val) = #param_name.map(|n| n as f64) {
                        if val > #max {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 {} 大于最大值 {}", #schema_name_str, val, #max)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    if val > #max {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 {} 大于最大值 {}", #schema_name_str, val, #max)
                        ));
                    }
                });
            }
        }

        if let Some(min_len) = p.min_length {
            if p.is_option {
                validations.push(quote! {
                    if let Some(ref val) = #param_name {
                        let len = val.len();
                        if len < #min_len {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的长度 {} 小于最小长度 {}", #schema_name_str, len, #min_len)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let len = #param_name.len();
                    if len < #min_len {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的长度 {} 小于最小长度 {}", #schema_name_str, len, #min_len)
                        ));
                    }
                });
            }
        }

        if let Some(max_len) = p.max_length {
            if p.is_option {
                validations.push(quote! {
                    if let Some(ref val) = #param_name {
                        let len = val.len();
                        if len > #max_len {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的长度 {} 大于最大长度 {}", #schema_name_str, len, #max_len)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let len = #param_name.len();
                    if len > #max_len {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的长度 {} 大于最大长度 {}", #schema_name_str, len, #max_len)
                        ));
                    }
                });
            }
        }

        if let Some(multiple) = p.multiple_of {
            if p.is_option {
                validations.push(quote! {
                    if let Some(val) = #param_name.map(|n| n as f64) {
                        let quotient = val / #multiple;
                        let remainder = (quotient - quotient.round()).abs();
                        if remainder > 0.0001 {
                            return Err(::tokitai::ToolError::validation_error(
                                format!("参数 '{}' 的值 {} 不是 {} 的倍数", #schema_name_str, val, #multiple)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    let quotient = val / #multiple;
                    let remainder = (quotient - quotient.round()).abs();
                    if remainder > 0.0001 {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("参数 '{}' 的值 {} 不是 {} 的倍数", #schema_name_str, val, #multiple)
                        ));
                    }
                });
            }
        }

        validations
    });

    let param_transforms = params.iter().filter_map(|p| {
        if let Some(transform_expr) = &p.transform {
            let param_name = &p.name;
            let transform_code = transform_expr.replace("value", &format!("{}", param_name));
            let transform_expr_tokens: Expr = match syn::parse_str(&transform_code) {
                Ok(expr) => expr,
                Err(e) => {
                    warn_if_not_test!(
                        "[tokitai] warning: failed to parse transform expression: {} - {}",
                        transform_code,
                        e
                    );
                    return None;
                }
            };
            Some(quote! {
                #param_name = #transform_expr_tokens;
            })
        } else {
            None
        }
    });

    let param_names: Vec<&Ident> = params.iter().map(|p| &p.name).collect();

    let result_handling = if tool.is_result {
        quote! {
            match result {
                Ok(v) => Ok(serde_json::to_value(v).unwrap()),
                Err(e) => Err(::tokitai::ToolError::internal_error(format!("{}", e))),
            }
        }
    } else {
        quote! {
            Ok(serde_json::to_value(result).unwrap())
        }
    };

    let fn_sig = if is_async {
        quote! { async fn #wrapper_name(&self, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> }
    } else {
        quote! { fn #wrapper_name(&self, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> }
    };

    let method_call = if tool.is_async && !is_async {
        quote! {
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => handle.block_on(async { self.#method_name(#(#param_names),*).await }),
                Err(_) => return Err(::tokitai::ToolError::internal_error(
                    "无法在同步上下文中调用异步工具：当前线程没有 tokio 运行时"
                )),
            }
        }
    } else {
        quote! { self.#method_name(#(#param_names),*) }
    };

    quote! {
        #[allow(clippy::all)]
        #fn_sig {
            use serde_json::Value;

            #(#param_parsing)*

            // 参数验证
            #(#param_validations)*

            // 参数转换
            #(#param_transforms)*

            let result = #method_call;
            #result_handling
        }
    }
}
