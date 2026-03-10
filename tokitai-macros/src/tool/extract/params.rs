//! 参数提取
//!
//! 包含 extract_params、parse_param_tool_attrs 等函数

use syn::{punctuated::Punctuated, token, Expr, ExprLit, FnArg, Lit, LitStr, Pat, PatType, Type};

use super::docs::{
    extract_param_attr_from_docs, extract_param_desc_from_docs, extract_param_transform_from_docs,
    extract_param_validate_from_docs, extract_validate_msg_from_docs,
};
use crate::tool::types::param::{ParamInfo, ParamToolAttrs};

/// 从函数签名提取参数
pub fn extract_params(
    inputs: &Punctuated<FnArg, token::Comma>,
    fn_attrs: &[syn::Attribute],
    hidden_params: &[String],
    param_validations: &[(String, ParamToolAttrs)],
) -> Vec<ParamInfo> {
    let mut params = Vec::new();

    let param_docs = super::docs::extract_param_docs(fn_attrs);

    for arg in inputs {
        if let FnArg::Typed(PatType { pat, ty, attrs, .. }) = arg {
            if let Pat::Ident(ident) = pat.as_ref() {
                if ident.ident == "self" || ident.ident == "_self" {
                    continue;
                }

                let param_name = ident.ident.to_string();

                if hidden_params.contains(&param_name) {
                    continue;
                }

                let schema_name = param_name
                    .strip_prefix('_')
                    .unwrap_or(&param_name)
                    .to_string();

                let mut param_tool_attrs = parse_param_tool_attrs(attrs).unwrap_or_default();

                if let Some(method_level_attrs) = param_validations
                    .iter()
                    .find(|(name, _)| name == &schema_name)
                {
                    if method_level_attrs.1.one_of.is_some() && param_tool_attrs.one_of.is_none() {
                        param_tool_attrs.one_of = method_level_attrs.1.one_of.clone();
                    }
                    if method_level_attrs.1.enum_values.is_some()
                        && param_tool_attrs.enum_values.is_none()
                    {
                        param_tool_attrs.enum_values = method_level_attrs.1.enum_values.clone();
                    }
                    if method_level_attrs.1.pattern.is_some() && param_tool_attrs.pattern.is_none()
                    {
                        param_tool_attrs.pattern = method_level_attrs.1.pattern.clone();
                    }
                    if method_level_attrs.1.min.is_some() && param_tool_attrs.min.is_none() {
                        param_tool_attrs.min = method_level_attrs.1.min;
                    }
                    if method_level_attrs.1.max.is_some() && param_tool_attrs.max.is_none() {
                        param_tool_attrs.max = method_level_attrs.1.max;
                    }
                    if method_level_attrs.1.min_length.is_some()
                        && param_tool_attrs.min_length.is_none()
                    {
                        param_tool_attrs.min_length = method_level_attrs.1.min_length;
                    }
                    if method_level_attrs.1.max_length.is_some()
                        && param_tool_attrs.max_length.is_none()
                    {
                        param_tool_attrs.max_length = method_level_attrs.1.max_length;
                    }
                    if method_level_attrs.1.min_items.is_some()
                        && param_tool_attrs.min_items.is_none()
                    {
                        param_tool_attrs.min_items = method_level_attrs.1.min_items;
                    }
                    if method_level_attrs.1.max_items.is_some()
                        && param_tool_attrs.max_items.is_none()
                    {
                        param_tool_attrs.max_items = method_level_attrs.1.max_items;
                    }
                    if method_level_attrs.1.multiple_of.is_some()
                        && param_tool_attrs.multiple_of.is_none()
                    {
                        param_tool_attrs.multiple_of = method_level_attrs.1.multiple_of;
                    }
                    if method_level_attrs.1.validate_msg.is_some()
                        && param_tool_attrs.validate_msg.is_none()
                    {
                        param_tool_attrs.validate_msg = method_level_attrs.1.validate_msg.clone();
                    }
                    if method_level_attrs.1.validate_msg_zh.is_some()
                        && param_tool_attrs.validate_msg_zh.is_none()
                    {
                        param_tool_attrs.validate_msg_zh =
                            method_level_attrs.1.validate_msg_zh.clone();
                    }
                    if method_level_attrs.1.validate_msg_en.is_some()
                        && param_tool_attrs.validate_msg_en.is_none()
                    {
                        param_tool_attrs.validate_msg_en =
                            method_level_attrs.1.validate_msg_en.clone();
                    }
                    if method_level_attrs.1.default.is_some() && param_tool_attrs.default.is_none()
                    {
                        param_tool_attrs.default = method_level_attrs.1.default.clone();
                    }
                    if method_level_attrs.1.example.is_some() && param_tool_attrs.example.is_none()
                    {
                        param_tool_attrs.example = method_level_attrs.1.example.clone();
                    }
                }

                let description = param_tool_attrs
                    .desc
                    .clone()
                    .or_else(|| extract_param_desc_from_docs(fn_attrs, &schema_name))
                    .or_else(|| param_docs.get(&schema_name).cloned())
                    .or_else(|| super::docs::extract_doc_comment(attrs));

                let is_required_explicit = param_tool_attrs.required
                    || extract_param_attr_from_docs(fn_attrs, &schema_name, "required");

                let example = param_tool_attrs.example.clone();
                let default = param_tool_attrs.default.clone();

                let validate = param_tool_attrs
                    .validate
                    .clone()
                    .or_else(|| extract_param_validate_from_docs(fn_attrs, &schema_name));
                let transform = param_tool_attrs
                    .transform
                    .clone()
                    .or_else(|| extract_param_transform_from_docs(fn_attrs, &schema_name));
                let validate_msg = param_tool_attrs
                    .validate_msg
                    .clone()
                    .or_else(|| extract_validate_msg_from_docs(fn_attrs, &schema_name));

                let one_of = param_tool_attrs.one_of.clone();
                let enum_values = param_tool_attrs.enum_values.clone();
                let pattern = param_tool_attrs.pattern.clone();
                let min = param_tool_attrs.min;
                let max = param_tool_attrs.max;
                let min_length = param_tool_attrs.min_length;
                let max_length = param_tool_attrs.max_length;
                let min_items = param_tool_attrs.min_items;
                let max_items = param_tool_attrs.max_items;
                let multiple_of = param_tool_attrs.multiple_of;

                let is_option = !is_required_explicit && is_option_type(ty);

                params.push(ParamInfo {
                    name: ident.ident.clone(),
                    schema_name,
                    ty: ty.as_ref().clone(),
                    description,
                    is_option,
                    is_required: is_required_explicit,
                    example,
                    default,
                    validate,
                    transform,
                    one_of,
                    enum_values,
                    pattern,
                    min,
                    max,
                    min_length,
                    max_length,
                    min_items,
                    max_items,
                    multiple_of,
                    validate_msg,
                    validate_msg_zh: param_tool_attrs.validate_msg_zh.clone(),
                    validate_msg_en: param_tool_attrs.validate_msg_en.clone(),
                });
            }
        }
    }

    params
}

/// 解析参数上的工具属性
pub fn parse_param_tool_attrs(attrs: &[syn::Attribute]) -> Option<ParamToolAttrs> {
    let mut result = ParamToolAttrs::default();
    let mut found_any = false;

    for attr in attrs {
        if attr.path().is_ident("param_tool") {
            if let Ok(args) = attr.parse_args::<ParamToolAttrs>() {
                if args.desc.is_some() {
                    result.desc = args.desc;
                    found_any = true;
                }
                if args.required {
                    result.required = true;
                    found_any = true;
                }
                if args.example.is_some() {
                    result.example = args.example;
                    found_any = true;
                }
                if args.default.is_some() {
                    result.default = args.default;
                    found_any = true;
                }
                if args.validate.is_some() {
                    result.validate = args.validate;
                    found_any = true;
                }
                if args.transform.is_some() {
                    result.transform = args.transform;
                    found_any = true;
                }
            }
        }

        if attr.path().is_ident("tool") {
            if let Ok(args) = attr.parse_args::<ParamToolAttrs>() {
                if args.desc.is_some() {
                    result.desc = args.desc;
                    found_any = true;
                }
                if args.required {
                    result.required = true;
                    found_any = true;
                }
                if args.example.is_some() {
                    result.example = args.example;
                    found_any = true;
                }
                if args.default.is_some() {
                    result.default = args.default;
                    found_any = true;
                }
                if args.validate.is_some() {
                    result.validate = args.validate;
                    found_any = true;
                }
                if args.transform.is_some() {
                    result.transform = args.transform;
                    found_any = true;
                }
            }
        }

        if attr.path().is_ident("tool_required") {
            result.required = true;
            found_any = true;
        } else if attr.path().is_ident("tool_desc") {
            let value = if let Ok(meta) = attr.parse_args::<LitStr>() {
                Some(meta.value())
            } else if let Ok(meta) = attr.parse_args::<ExprLit>() {
                if let Lit::Str(lit) = &meta.lit {
                    Some(lit.value())
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(v) = value {
                result.desc = Some(v);
                found_any = true;
            }
        } else if attr.path().is_ident("tool_example") {
            if let Ok(_meta) = attr.parse_args::<Expr>() {
                if let Ok(meta) = attr.parse_args::<LitStr>() {
                    result.example =
                        serde_json::from_str::<serde_json::Value>(&format!("\"{}\"", meta.value()))
                            .ok();
                    found_any = true;
                } else if let Ok(meta) = attr.parse_args::<Lit>() {
                    result.example = parse_literal_to_json(&meta);
                    found_any = true;
                }
            }
        } else if attr.path().is_ident("tool_default") {
            if let Ok(_meta) = attr.parse_args::<Expr>() {
                if let Ok(meta) = attr.parse_args::<LitStr>() {
                    result.default =
                        serde_json::from_str::<serde_json::Value>(&format!("\"{}\"", meta.value()))
                            .ok();
                    found_any = true;
                } else if let Ok(meta) = attr.parse_args::<Lit>() {
                    result.default = parse_literal_to_json(&meta);
                    found_any = true;
                }
            }
        } else if attr.path().is_ident("tool_validate") {
            if let Ok(meta) = attr.parse_args::<LitStr>() {
                result.validate = Some(meta.value());
                found_any = true;
            } else if let Ok(meta) = attr.parse_args::<ExprLit>() {
                if let Lit::Str(lit) = &meta.lit {
                    result.validate = Some(lit.value());
                    found_any = true;
                }
            }
        } else if attr.path().is_ident("tool_transform") {
            if let Ok(meta) = attr.parse_args::<LitStr>() {
                result.transform = Some(meta.value());
                found_any = true;
            } else if let Ok(meta) = attr.parse_args::<ExprLit>() {
                if let Lit::Str(lit) = &meta.lit {
                    result.transform = Some(lit.value());
                    found_any = true;
                }
            }
        }
    }

    if found_any {
        Some(result)
    } else {
        None
    }
}

/// 将字面量解析为 JSON 值
fn parse_literal_to_json(lit: &Lit) -> Option<serde_json::Value> {
    match lit {
        Lit::Str(lit_str) => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&lit_str.value()) {
                Some(val)
            } else {
                Some(serde_json::Value::String(lit_str.value()))
            }
        }
        Lit::Int(lit_int) => lit_int
            .base10_parse::<i64>()
            .ok()
            .map(|v| serde_json::json!(v)),
        Lit::Float(lit_float) => lit_float
            .base10_parse::<f64>()
            .ok()
            .map(|v| serde_json::json!(v)),
        Lit::Bool(lit_bool) => Some(serde_json::json!(lit_bool.value)),
        _ => None,
    }
}

/// 检查类型是否为 Option
pub fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.first() {
            return segment.ident == "Option";
        }
    }
    false
}

/// 检查返回类型是否为 Result
pub fn is_result_type(output: &syn::ReturnType) -> bool {
    match output {
        syn::ReturnType::Type(_, ty) => {
            if let Type::Path(path) = ty.as_ref() {
                return path
                    .path
                    .segments
                    .first()
                    .map(|s| s.ident == "Result")
                    .unwrap_or(false);
            }
            false
        }
        _ => false,
    }
}
