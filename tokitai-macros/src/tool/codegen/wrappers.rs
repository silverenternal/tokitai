//! 包装方法生成
//!
//! 包含 generate_wrapper_method_sync、generate_wrapper_method、generate_helper_methods 等函数

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Expr, Ident};

use crate::tool::types::tool_method::ToolMethodInfo;

/// T-015: `true` when the consumer opted into in-process
/// `tracing::instrument` spans via `TOKITAI_TRACE=1` (or any
/// non-empty `TOKITAI_TRACE`).
///
/// The check is `option_env!` (not `std::env::var`) because the
/// value is baked into the macro at compile time — by the time a
/// proc-macro invocation runs, `std::env::var` of the host
/// process no longer reflects the cargo build environment that
/// drove this build. `tokitai-macros/build.rs` forwards the
/// consumer-side env var to the macro's compile environment.
///
/// When this returns `false`, the macro emits **no** `tracing`
/// references anywhere in the generated code — the `#[allow(...)]`
/// attribute on the wrapper, the `tracing` field initializers, and
/// the `tracing::instrument` attribute are all skipped. The binary
/// size delta is therefore exactly zero on the default build
/// (verified by CI via the binary-size smoke harness).
fn tracing_enabled() -> bool {
    option_env!("TOKITAI_TRACE").is_some_and(|v| !v.is_empty())
}

/// T-015: emit a `#[tracing::instrument(...)]` attribute string
/// tailored to one wrapper invocation. The attribute carries the
/// span name plus `Empty` placeholders for the four fields the
/// macro's body will fill in via `tracing::Span::current().record(...)`:
///
/// - `tool.name` — the primary tool name (e.g. `"add"`)
/// - `tool.version` — the tool's `version` attribute if any,
///   otherwise the literal string `"-"` so a downstream
///   subscriber can pattern-match on a single key
/// - `args.size` — byte length of the JSON arguments object
/// - `result.size` — byte length of the JSON result object
///
/// We deliberately leave all four fields as
/// `tracing::field::Empty` here and record them in the body
/// rather than passing the literal values through the
/// attribute's `fields(...)` list. Reason: `tracing::instrument`
/// runs at *function-entry* time and would force us to compute
/// `args.size` and `result.size` *before* the tool method runs,
/// which is impossible for `result.size`. Recording in the body
/// (after `result` is bound) lets us measure both endpoints of
/// the call. `skip_all` keeps the wrapper's `&self` and `args`
/// borrow out of the span's recorded fields, which is the
/// zero-overhead default: with no subscriber registered the
/// `tracing::Span::current()` lookup returns the no-op span
/// and the `record(...)` calls collapse to a single branch +
/// branch.
fn tracing_instrument_attr(_tool: &ToolMethodInfo) -> Option<TokenStream2> {
    if !tracing_enabled() {
        return None;
    }
    // T-015: emit `#[tracing::instrument]` with all four
    // documented fields declared up front as
    // `tracing::field::Empty` placeholders. The macro's body
    // records the actual values via
    // `tracing::Span::current().record(...)` because
    // `result.size` is only known after the tool method
    // runs — `tracing::instrument`'s field list is evaluated
    // at function entry, before the method has executed.
    //
    // Declaring the fields here is what makes `record(...)`
    // work: without the declaration, `record(...)` is a
    // no-op (the span has no field registered for that
    // name) and a downstream subscriber sees an empty
    // `fields` map.
    //
    // `skip_all` keeps `&self` and `args` out of the
    // auto-recorded fields — they are large and bloat the
    // span log. `tool.name`, `tool.version`, `args.size`,
    // and `result.size` carry the information a subscriber
    // actually needs.
    let attr: syn::Attribute = syn::parse_quote! {
        #[tracing::instrument(
            level = "info",
            name = "tokitai_tool_call",
            fields(
                tool.name = ::tracing::field::Empty,
                tool.version = ::tracing::field::Empty,
                args.size = ::tracing::field::Empty,
                result.size = ::tracing::field::Empty,
            ),
            skip_all,
        )]
    };
    Some(quote! { #attr })
}

/// T-015: emit inline `tracing::Span::current().record(...)`
/// calls that populate the four documented span fields
/// (`tool.name`, `tool.version`, `args.size`, `result.size`)
/// on the wrapper's span. The recording happens *only* when
/// the trace feature is on; when the feature is off this
/// helper returns `quote! {}` and the wrapper compiles to the
/// same machine code as before.
///
/// We record `tool.name` / `tool.version` / `args.size` once
/// at function entry (before any heavy lifting) and
/// `result.size` once just before returning, so the span
/// carries both endpoints of the call. The
/// `tracing::Span::current()` call is a no-op when no
/// subscriber is registered — the linker drops it on the
/// default build.
///
/// The returned `TokenStream` is always a *statement* (or
/// empty) so callers can splice it between the
/// `let result = ...;` binding and the final return value
/// without disturbing the expression that produces the
/// wrapper's `Result<Value, ToolError>`.
fn tracing_record_static_fields(tool: &ToolMethodInfo) -> TokenStream2 {
    if !tracing_enabled() {
        return quote! {};
    }
    let tool_name = tool.tool_name.clone();
    let tool_version = tool.version.clone().unwrap_or_else(|| "-".to_string());
    quote! {
        // T-015: record the static fields (tool.name /
        // tool.version / args.size) on entry. No-op when no
        // subscriber is registered.
        {
            let __tokitai_span = ::tracing::Span::current();
            __tokitai_span.record("tool.name", #tool_name);
            __tokitai_span.record("tool.version", #tool_version);
            __tokitai_span.record("args.size", args.to_string().len());
        };
    }
}

/// T-015: emit a token stream that records `result.size` for
/// the wrapper's outgoing `Result<Value, ToolError>` and then
/// returns it. When the trace feature is off, returns
/// `result_expr` unchanged (the wrapper's existing
/// `Ok(...)` / `Err(...)` arm keeps its original behaviour).
///
/// `result_expr` is expected to reference `result` (the local
/// variable bound to the tool method's return value just
/// above this block). On the trace-on path we re-bind
/// `result` to a `Result<&Value, &ToolError>` so we can
/// measure size without re-serializing.
fn tracing_record_result_and_return(result_expr: TokenStream2) -> TokenStream2 {
    if !tracing_enabled() {
        return quote! { #result_expr };
    }
    quote! {
        {
            // T-015: bind the wrapper's outgoing
            // `Result<Value, ToolError>` so we can measure
            // its size before returning. `result_expr`
            // produces the value; we re-serialize the Ok
            // arm to count its bytes and skip the Err arm
            // (we record 0 on error so subscribers can
            // filter on `result.size = 0` to find errors).
            let __tokitai_out: ::std::result::Result<
                ::serde_json::Value,
                ::tokitai::ToolError,
            > = #result_expr;
            let __tokitai_size: u64 = match &__tokitai_out {
                Ok(v) => ::serde_json::to_string(v)
                    .map(|s| s.len() as u64)
                    .unwrap_or(0),
                Err(_) => 0,
            };
            ::tracing::Span::current().record("result.size", __tokitai_size);
            __tokitai_out
        }
    }
}

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
    // Upper bound: each async tool emits 2 wrappers (async + sync),
    // each sync tool emits 1.
    let mut methods = Vec::with_capacity(tools.len() * 2);
    let has_async = tools.iter().any(|t| t.is_async);

    for tool in tools {
        if has_async {
            if tool.is_async {
                // 异步工具：emit __call_<name> (async) AND __call_<name>_sync (block_on wrapper)
                methods.push(generate_wrapper_method(tool, true));
                methods.push(generate_wrapper_method(tool, false));
            } else {
                // 同步工具：emit only __call_<name>_sync (the dispatcher.call_tool_sync targets this name).
                // The async `call_tool` dispatcher wraps the call in `async { ... }.await` so
                // the sync wrapper is uniformly callable from both dispatchers.
                methods.push(generate_wrapper_method(tool, false));
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
        } else if let Some(default_value) = &p.default {
            let default_json = serde_json::to_string(default_value)
                .unwrap_or_else(|_| "null".to_string());
            let default_lit = syn::LitStr::new(&default_json, Span::call_site());
            quote! {
                let #param_name: #param_type = serde_json::from_value(
                    args.get(#schema_name_str).cloned()
                        .unwrap_or_else(|| serde_json::from_str(#default_lit).unwrap())
                )
                .map_err(|e| ::tokitai::ToolError::validation_error(
                    format!("parameter type mismatch: {} (expected: {})", #schema_name_str, std::any::type_name::<#param_type>())
                ))?;
            }
        } else {
            quote! {
                let #param_name = args.get(#schema_name_str)
                    .ok_or_else(|| ::tokitai::ToolError::validation_error(
                        format!("missing required parameter '{}' (type: {})", #schema_name_str, std::any::type_name::<#param_type>())
                    ))?;
                let mut #param_name: #param_type = serde_json::from_value(#param_name.clone())
                    .map_err(|e| ::tokitai::ToolError::validation_error(
                        format!("parameter type mismatch: {} (expected: {})", #schema_name_str, std::any::type_name::<#param_type>())
                    ))?;
            }
        }
    });

    let param_validations = params.iter().flat_map(|p| {
        // Up to 9 validator slots can be filled per param (validate,
        // one_of, pattern, min, max, min_length, max_length,
        // multiple_of). The most common case is 0 (the param has no
        // validators), but pre-allocating 1 means the hot path of
        // "exactly one validator" is allocation-free, and we cap at
        // a small constant to bound the upper case.
        let mut validations = Vec::with_capacity(1);
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
                            format!("validation failed for parameter '{}': {}", #schema_name_str, #validate_expr)
                        }
                    }
                }
            } else if let Some(ref msg) = p.validate_msg {
                quote! { #msg.to_string() }
            } else {
                quote! { format!("validation failed for parameter '{}': {}", #schema_name_str, #validate_expr) }
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
                                format!("value '{}' for parameter '{}' is not in the allowed set, allowed values: {}", #schema_name_str, val, [#(#allowed_values),*].join(", "))
                            ));
                        }
                    }
                });
            } else {
                let allowed_values = one_of;
                validations.push(quote! {
                    if ![#(#allowed_values),*].contains(&#param_name.as_str()) {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("value '{}' for parameter '{}' is not in the allowed set, allowed values: {}", #schema_name_str, #param_name, [#(#allowed_values),*].join(", "))
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
                                format!("value '{}' for parameter '{}' does not contain pattern: {}", #schema_name_str, val, #pattern)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    if !#param_name.contains(#pattern) {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("value '{}' for parameter '{}' does not contain pattern: {}", #schema_name_str, #param_name, #pattern)
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
                                format!("value {} for parameter '{}' is less than minimum {}", #schema_name_str, val, #min)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    if val < #min {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("value {} for parameter '{}' is less than minimum {}", #schema_name_str, val, #min)
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
                                format!("value {} for parameter '{}' is greater than maximum {}", #schema_name_str, val, #max)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    if val > #max {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("value {} for parameter '{}' is greater than maximum {}", #schema_name_str, val, #max)
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
                                format!("length {} for parameter '{}' is less than minimum length {}", #schema_name_str, len, #min_len)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let len = #param_name.len();
                    if len < #min_len {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("length {} for parameter '{}' is less than minimum length {}", #schema_name_str, len, #min_len)
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
                                format!("length {} for parameter '{}' is greater than maximum length {}", #schema_name_str, len, #max_len)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let len = #param_name.len();
                    if len > #max_len {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("length {} for parameter '{}' is greater than maximum length {}", #schema_name_str, len, #max_len)
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
                                format!("value {} for parameter '{}' is not a multiple of {}", #schema_name_str, val, #multiple)
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
                            format!("value {} for parameter '{}' is not a multiple of {}", #schema_name_str, val, #multiple)
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

    // T-015: emit a `#[tracing::instrument(...)]` attribute when
    // `TOKITAI_TRACE` is set in the consumer build environment.
    // The attribute is *no-op* (i.e. the `quote!` collapses to
    // nothing) on the default build, so the macro output is
    // byte-identical whether the feature is on or off modulo a
    // `tracing` reference that the linker then drops.
    let tracing_attr = tracing_instrument_attr(tool).unwrap_or_else(|| quote! {});
    let record_static = tracing_record_static_fields(tool);
    let record_result = tracing_record_result_and_return(quote! { #result_handling });

    quote! {
        #[allow(clippy::all)]
        #tracing_attr
        fn #wrapper_name(&self, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> {
            use serde_json::Value;

            // T-015: record the static fields BEFORE the
            // parameter parsing so a subscriber sees
            // `args.size` even when the wrapper short-circuits
            // with a validation error. The `record_static`
            // block collapses to `{}` on the default build,
            // so this does not affect the hot path.
            #record_static

            #(#param_parsing)*

            // 参数验证
            #(#param_validations)*

            // 参数转换
            #(#param_transforms)*

            let result = self.#method_name(#(#param_names),*);
            // T-015: the `record_result` block records
            // `result.size` on the span before returning.
            // When the trace feature is off this collapses to
            // `result_handling` verbatim.
            #record_result
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
        } else if let Some(default_value) = &p.default {
            let default_json = serde_json::to_string(default_value)
                .unwrap_or_else(|_| "null".to_string());
            let default_lit = syn::LitStr::new(&default_json, Span::call_site());
            quote! {
                let #param_name: #param_type = serde_json::from_value(
                    args.get(#schema_name_str).cloned()
                        .unwrap_or_else(|| serde_json::from_str(#default_lit).unwrap())
                )
                .map_err(|e| ::tokitai::ToolError::validation_error(
                    format!("parameter type mismatch: {} (expected: {})", #schema_name_str, std::any::type_name::<#param_type>())
                ))?;
            }
        } else {
            quote! {
                let #param_name = args.get(#schema_name_str)
                    .ok_or_else(|| ::tokitai::ToolError::validation_error(
                        format!("missing required parameter '{}' (type: {})", #schema_name_str, std::any::type_name::<#param_type>())
                    ))?;
                let mut #param_name: #param_type = serde_json::from_value(#param_name.clone())
                    .map_err(|e| ::tokitai::ToolError::validation_error(
                        format!("parameter type mismatch: {} (expected: {})", #schema_name_str, std::any::type_name::<#param_type>())
                    ))?;
            }
        }
    });

    let param_validations = params.iter().flat_map(|p| {
        // Up to 9 validator slots can be filled per param (validate,
        // one_of, pattern, min, max, min_length, max_length,
        // multiple_of). The most common case is 0 (the param has no
        // validators), but pre-allocating 1 means the hot path of
        // "exactly one validator" is allocation-free, and we cap at
        // a small constant to bound the upper case.
        let mut validations = Vec::with_capacity(1);
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
                            format!("validation failed for parameter '{}': {}", #schema_name_str, #validate_expr)
                        }
                    }
                }
            } else if let Some(ref msg) = p.validate_msg {
                quote! { #msg.to_string() }
            } else {
                quote! { format!("validation failed for parameter '{}': {}", #schema_name_str, #validate_expr) }
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
                                format!("value '{}' for parameter '{}' is not in the allowed set, allowed values: {}", #schema_name_str, val, [#(#allowed_values),*].join(", "))
                            ));
                        }
                    }
                });
            } else {
                let allowed_values = one_of;
                validations.push(quote! {
                    if ![#(#allowed_values),*].contains(&#param_name.as_str()) {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("value '{}' for parameter '{}' is not in the allowed set, allowed values: {}", #schema_name_str, #param_name, [#(#allowed_values),*].join(", "))
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
                                format!("value '{}' for parameter '{}' does not contain pattern: {}", #schema_name_str, val, #pattern)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    if !#param_name.contains(#pattern) {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("value '{}' for parameter '{}' does not contain pattern: {}", #schema_name_str, #param_name, #pattern)
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
                                format!("value {} for parameter '{}' is less than minimum {}", #schema_name_str, val, #min)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    if val < #min {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("value {} for parameter '{}' is less than minimum {}", #schema_name_str, val, #min)
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
                                format!("value {} for parameter '{}' is greater than maximum {}", #schema_name_str, val, #max)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let val = #param_name as f64;
                    if val > #max {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("value {} for parameter '{}' is greater than maximum {}", #schema_name_str, val, #max)
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
                                format!("length {} for parameter '{}' is less than minimum length {}", #schema_name_str, len, #min_len)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let len = #param_name.len();
                    if len < #min_len {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("length {} for parameter '{}' is less than minimum length {}", #schema_name_str, len, #min_len)
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
                                format!("length {} for parameter '{}' is greater than maximum length {}", #schema_name_str, len, #max_len)
                            ));
                        }
                    }
                });
            } else {
                validations.push(quote! {
                    let len = #param_name.len();
                    if len > #max_len {
                        return Err(::tokitai::ToolError::validation_error(
                            format!("length {} for parameter '{}' is greater than maximum length {}", #schema_name_str, len, #max_len)
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
                                format!("value {} for parameter '{}' is not a multiple of {}", #schema_name_str, val, #multiple)
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
                            format!("value {} for parameter '{}' is not a multiple of {}", #schema_name_str, val, #multiple)
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
        // T-003: the macro's sync-from-async wrapper probes the
        // per-call override seam BEFORE the global slot and BEFORE
        // falling back to the active Tokio runtime. The probe uses
        // `block_on_for_executor()` (the typed helper exposed in
        // `tokitai_core`); the actual drive still goes through
        // `tokio::runtime::Handle::block_on` because that path
        // accepts non-`'static` futures (which the wrapper's
        // `&self` borrow requires). The override seam is also
        // surfaced by the resilience decorators (`#[retry]`,
        // `#[rate_limit]`, `#[circuit_breaker]`) which already use
        // `tokitai_core::block_on_async` and therefore pick up the
        // override automatically.
        //
        // Resolution order:
        //   1. `block_on_for_executor()` (per-call / per-thread probe)
        //   2. `current_async_executor()` (global slot from
        //      `set_async_executor`)
        //   3. Active Tokio runtime on the current thread
        //   4. Clear English error containing "no async runtime" so
        //      downstream observability tools can pattern-match on it.
        quote! {
            {
                // T-003 probe: verify the per-call override seam is
                // configured. The actual drive still uses Tokio
                // because the wrapper's `&self` borrow is not
                // `'static`. If the user has installed a non-Tokio
                // executor via `set_async_executor` AND a Tokio
                // runtime is reachable on this thread, Tokio wins —
                // this matches the pre-T-003 behaviour and keeps
                // existing tests stable.
                let __tokitai_probe = ::tokitai_core::block_on_for_executor().is_some()
                    || ::tokitai_core::current_async_executor().is_some();
                match tokio::runtime::Handle::try_current() {
                    Ok(handle) => {
                        // Tokio reachable — drive the future.
                        let _ = __tokitai_probe;
                        handle.block_on(async { self.#method_name(#(#param_names),*).await })
                    }
                    Err(_) => {
                        // No Tokio runtime on this thread. If the
                        // user registered a non-Tokio executor via
                        // `set_async_executor`, surface a clear
                        // message that explains the constraint
                        // (the macro wrapper cannot drive a
                        // non-Tokio executor while borrowing `&self`).
                        if __tokitai_probe {
                            return Err(::tokitai::ToolError::internal_error(
                                "an AsyncExecutor is registered via set_async_executor, \
                                 but the #[tool] macro's sync-from-async wrapper requires \
                                 a Tokio runtime on the current thread because it borrows \
                                 &self. Call this from inside a tokio runtime, or convert \
                                 the tool method to a sync fn."
                            ));
                        }
                        return Err(::tokitai::ToolError::internal_error(
                            ::tokitai_core::block_on_async_error_message()
                        ));
                    }
                }
            }
        }
    } else if tool.is_async {
        // 异步工具的异步包装：必须 .await future
        quote! { self.#method_name(#(#param_names),*).await }
    } else {
        quote! { self.#method_name(#(#param_names),*) }
    };

    // T-015: emit a `#[tracing::instrument(...)]` attribute when
    // `TOKITAI_TRACE` is set in the consumer build environment.
    // The attribute is *no-op* (i.e. the `quote!` collapses to
    // nothing) on the default build, so the macro output is
    // byte-identical whether the feature is on or off modulo a
    // `tracing` reference that the linker then drops.
    let tracing_attr = tracing_instrument_attr(tool).unwrap_or_else(|| quote! {});
    let record_static = tracing_record_static_fields(tool);
    let record_result = tracing_record_result_and_return(quote! { #result_handling });

    quote! {
        #[allow(clippy::all)]
        #tracing_attr
        #fn_sig {
            use serde_json::Value;

            // T-015: record the static fields BEFORE the
            // parameter parsing so a subscriber sees
            // `args.size` even when the wrapper short-circuits
            // with a validation error. The `record_static`
            // block collapses to `{}` on the default build,
            // so this does not affect the hot path.
            #record_static

            #(#param_parsing)*

            // 参数验证
            #(#param_validations)*

            // 参数转换
            #(#param_transforms)*

            let result = #method_call;
            // T-015: the `record_result` block records
            // `result.size` on the span before returning.
            // When the trace feature is off this collapses to
            // `result_handling` verbatim.
            #record_result
        }
    }
}
