//! 参数信息数据结构

use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream},
    token, Expr, Ident, Lit, LitStr, Type,
};

/// 参数级别的工具属性
#[derive(Default, Clone)]
pub struct ParamToolAttrs {
    pub desc: Option<String>,
    pub required: bool,
    pub example: Option<serde_json::Value>,
    pub default: Option<serde_json::Value>,
    pub validate: Option<String>,
    pub transform: Option<String>,
    // JSON Schema 验证属性
    pub one_of: Option<Vec<String>>,
    pub enum_values: Option<Vec<serde_json::Value>>,
    pub pattern: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub min_items: Option<usize>,
    pub max_items: Option<usize>,
    pub multiple_of: Option<f64>,
    pub validate_msg: Option<String>,
    pub validate_msg_zh: Option<String>,
    pub validate_msg_en: Option<String>,
}

impl Parse for ParamToolAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut desc = None;
        let mut required = false;
        let mut example = None;
        let mut default = None;
        let mut validate = None;
        let mut transform = None;
        let mut one_of: Option<Vec<String>> = None;
        let mut enum_values: Option<Vec<serde_json::Value>> = None;
        let mut pattern = None;
        let mut min: Option<f64> = None;
        let mut max: Option<f64> = None;
        let mut min_length: Option<usize> = None;
        let mut max_length: Option<usize> = None;
        let mut min_items: Option<usize> = None;
        let mut max_items: Option<usize> = None;
        let mut multiple_of: Option<f64> = None;
        let mut validate_msg = None;
        let mut validate_msg_zh = None;
        let mut validate_msg_en = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            match key.to_string().as_str() {
                "desc" | "description" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    desc = Some(value.value());
                }
                "required" => {
                    required = true;
                }
                "example" => {
                    input.parse::<token::Eq>()?;
                    example = parse_json_value(input)?;
                }
                "default" => {
                    input.parse::<token::Eq>()?;
                    default = parse_json_value(input)?;
                }
                "validate" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    validate = Some(value.value());
                }
                "transform" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    transform = Some(value.value());
                }
                "one_of" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    let mut values = Vec::new();
                    while !content.is_empty() {
                        let val: LitStr = content.parse()?;
                        values.push(val.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                    one_of = Some(values);
                }
                "enum_values" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    let mut values = Vec::new();
                    while !content.is_empty() {
                        let val_expr: Expr = content.parse()?;
                        let val_str = val_expr.to_token_stream().to_string();
                        values.push(parse_value_string(&val_str));
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                    enum_values = Some(values);
                }
                "pattern" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    pattern = Some(value.value());
                }
                "min" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    min = value.value().parse().ok();
                }
                "max" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    max = value.value().parse().ok();
                }
                "min_length" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    min_length = value.value().parse().ok();
                }
                "max_length" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    max_length = value.value().parse().ok();
                }
                "min_items" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    min_items = value.value().parse().ok();
                }
                "max_items" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    max_items = value.value().parse().ok();
                }
                "multiple_of" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    multiple_of = value.value().parse().ok();
                }
                "validate_msg" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    validate_msg = Some(value.value());
                }
                "validate_msg_zh" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    validate_msg_zh = Some(value.value());
                }
                "validate_msg_en" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    validate_msg_en = Some(value.value());
                }
                _ => {
                    if input.peek(token::Eq) {
                        input.parse::<token::Eq>()?;
                        let _: LitStr = input.parse()?;
                    }
                }
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        Ok(ParamToolAttrs {
            desc,
            required,
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
            validate_msg_zh,
            validate_msg_en,
        })
    }
}

/// 解析 JSON 值（支持字符串字面量或任意 Rust 字面量）
pub fn parse_json_value(input: ParseStream) -> syn::Result<Option<serde_json::Value>> {
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

    if input.peek(syn::token::Brace) {
        let content;
        syn::braced!(content in input);
        let mut map = serde_json::Map::new();

        while !content.is_empty() {
            let key: LitStr = content.parse()?;
            content.parse::<syn::token::Colon>()?;

            let value_expr: Expr = content.parse()?;
            let value_str = value_expr.to_token_stream().to_string();

            let json_value = parse_value_string(&value_str);
            map.insert(key.value(), json_value);

            if content.peek(syn::token::Comma) {
                content.parse::<syn::token::Comma>()?;
            }
        }

        return Ok(Some(serde_json::Value::Object(map)));
    }

    if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let mut arr = Vec::new();

        while !content.is_empty() {
            let value_expr: Expr = content.parse()?;
            let value_str = value_expr.to_token_stream().to_string();
            arr.push(parse_value_string(&value_str));

            if content.peek(syn::token::Comma) {
                content.parse::<syn::token::Comma>()?;
            }
        }

        return Ok(Some(serde_json::Value::Array(arr)));
    }

    if let Ok(lit) = input.parse::<Lit>() {
        match lit {
            Lit::Str(lit_str) => {
                let str_value = lit_str.value();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&str_value) {
                    return Ok(Some(val));
                }
                Ok(Some(serde_json::Value::String(str_value)))
            }
            Lit::Int(lit_int) => {
                if let Ok(val) = lit_int.base10_parse::<i64>() {
                    Ok(Some(serde_json::json!(val)))
                } else {
                    Ok(None)
                }
            }
            Lit::Float(lit_float) => {
                if let Ok(val) = lit_float.base10_parse::<f64>() {
                    Ok(Some(serde_json::json!(val)))
                } else {
                    Ok(None)
                }
            }
            Lit::Bool(lit_bool) => Ok(Some(serde_json::json!(lit_bool.value))),
            _ => Ok(None),
        }
    } else {
        Ok(None)
    }
}

/// 将值字符串解析为 JSON 值
pub fn parse_value_string(s: &str) -> serde_json::Value {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(s) {
        return val;
    }

    if let Some(inner) = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return serde_json::Value::String(inner.to_string());
    }

    if let Ok(val) = s.parse::<i64>() {
        return serde_json::json!(val);
    }

    if let Ok(val) = s.parse::<f64>() {
        return serde_json::json!(val);
    }

    match s {
        "true" => return serde_json::json!(true),
        "false" => return serde_json::json!(false),
        _ => {}
    }

    serde_json::Value::String(s.to_string())
}

/// 从输入中解析字面量为 f64
pub fn parse_lit_to_f64(input: ParseStream) -> syn::Result<Option<f64>> {
    if input.peek(syn::LitInt) {
        let lit_int: syn::LitInt = input.parse()?;
        return lit_int.base10_parse::<f64>().map(Some).or(Ok(None));
    }
    if input.peek(syn::LitFloat) {
        let lit_float: syn::LitFloat = input.parse()?;
        return lit_float.base10_parse::<f64>().map(Some).or(Ok(None));
    }
    if input.peek(LitStr) {
        let lit_str: LitStr = input.parse()?;
        return Ok(lit_str.value().parse::<f64>().ok());
    }
    Ok(None)
}

/// 从输入中解析字面量为 usize
pub fn parse_lit_to_usize(input: ParseStream) -> syn::Result<Option<usize>> {
    if input.peek(syn::LitInt) {
        let lit_int: syn::LitInt = input.parse()?;
        return lit_int.base10_parse::<usize>().map(Some).or(Ok(None));
    }
    if input.peek(LitStr) {
        let lit_str: LitStr = input.parse()?;
        return Ok(lit_str.value().parse::<usize>().ok());
    }
    Ok(None)
}

/// 从输入中解析字面量为字符串
pub fn parse_lit_to_string(input: ParseStream) -> syn::Result<Option<String>> {
    if input.peek(LitStr) {
        let lit_str: LitStr = input.parse()?;
        return Ok(Some(lit_str.value()));
    }
    Ok(None)
}

/// 参数信息
#[allow(dead_code)]
#[derive(Clone)]
pub struct ParamInfo {
    pub name: Ident,
    pub schema_name: String,
    pub ty: Type,
    pub description: Option<String>,
    pub is_option: bool,
    pub is_required: bool,
    pub example: Option<serde_json::Value>,
    pub default: Option<serde_json::Value>,
    pub validate: Option<String>,
    pub transform: Option<String>,
    // JSON Schema 验证属性
    pub one_of: Option<Vec<String>>,
    pub enum_values: Option<Vec<serde_json::Value>>,
    pub pattern: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub min_items: Option<usize>,
    pub max_items: Option<usize>,
    pub multiple_of: Option<f64>,
    pub validate_msg: Option<String>,
    pub validate_msg_zh: Option<String>,
    pub validate_msg_en: Option<String>,
}

impl ParamInfo {
    /// 创建新的 ParamInfo
    pub fn new(name: Ident, ty: Type) -> Self {
        let schema_name = name.to_string();
        Self {
            name,
            schema_name,
            ty,
            description: None,
            is_option: false,
            is_required: false,
            example: None,
            default: None,
            validate: None,
            transform: None,
            one_of: None,
            enum_values: None,
            pattern: None,
            min: None,
            max: None,
            min_length: None,
            max_length: None,
            min_items: None,
            max_items: None,
            multiple_of: None,
            validate_msg: None,
            validate_msg_zh: None,
            validate_msg_en: None,
        }
    }
}
