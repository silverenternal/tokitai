//! `#[tool]` 宏实现
//!
//! 核心设计：
//! 1. 单一宏同时处理 impl 块和方法
//! 2. 编译期生成所有工具定义
//! 3. 使用 JsonSchema AST + serde_json 生成规范的 JSON Schema
//! 4. 支持自定义 struct 字段解析

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    token, Expr, ExprLit, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, ItemStruct, Lit, LitStr,
    Meta, Pat, PatType, ReturnType, Type, Visibility,
};

/// 参数级别的工具属性
#[derive(Default, Clone)]
struct ParamToolAttrs {
    desc: Option<String>,
    required: bool,
    example: Option<serde_json::Value>, // 改为支持任意 JSON 值
    default: Option<serde_json::Value>, // 改为支持任意 JSON 值
    validate: Option<String>,           // 验证表达式
    transform: Option<String>,          // 转换表达式
    // JSON Schema 验证属性
    one_of: Option<Vec<String>>,                 // 枚举值（字符串）
    enum_values: Option<Vec<serde_json::Value>>, // 枚举值（任意类型）
    pattern: Option<String>,                     // 正则表达式
    min: Option<f64>,                            // 数值最小值
    max: Option<f64>,                            // 数值最大值
    min_length: Option<usize>,                   // 字符串/数组最小长度
    max_length: Option<usize>,                   // 字符串/数组最大长度
    min_items: Option<usize>,                    // 数组最小项数
    max_items: Option<usize>,                    // 数组最大项数
    multiple_of: Option<f64>,                    // 倍数限制
    validate_msg: Option<String>,                // 自定义验证错误消息
    validate_msg_zh: Option<String>,             // 中文验证错误消息
    validate_msg_en: Option<String>,             // 英文验证错误消息
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
                    // 支持字符串或任意字面量
                    example = parse_json_value(input)?;
                }
                "default" => {
                    input.parse::<token::Eq>()?;
                    // 支持字符串或任意字面量
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
                    // 解析 one_of = ["admin", "user", "guest"]
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
                    // 解析 enum_values = [1, 2, 3] 或 ["a", "b"]
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
                    // 跳过未知属性
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
fn parse_json_value(input: ParseStream) -> syn::Result<Option<serde_json::Value>> {
    // 尝试解析为其他字面量（整数、浮点数、布尔值等）- 优先处理
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

    // 尝试解析为字符串字面量
    if let Ok(lit_str) = input.parse::<LitStr>() {
        let str_value = lit_str.value();
        // 首先尝试直接解析字符串内容为 JSON（如 "null", "42", "true", "[1,2]" 等）
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&str_value) {
            return Ok(Some(val));
        }
        // 如果失败，作为普通字符串处理
        return Ok(Some(serde_json::Value::String(str_value)));
    }

    // 尝试解析为对象字面量 { ... }
    if input.peek(syn::token::Brace) {
        let content;
        syn::braced!(content in input);
        let mut map = serde_json::Map::new();

        while !content.is_empty() {
            let key: LitStr = content.parse()?;
            content.parse::<syn::token::Colon>()?;

            // 解析值为任意表达式
            let value_expr: Expr = content.parse()?;
            let value_str = value_expr.to_token_stream().to_string();

            // 尝试将值解析为 JSON
            let json_value = parse_value_string(&value_str);
            map.insert(key.value(), json_value);

            if content.peek(syn::token::Comma) {
                content.parse::<syn::token::Comma>()?;
            }
        }

        return Ok(Some(serde_json::Value::Object(map)));
    }

    // 尝试解析为数组字面量 [ ... ]
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

    // 尝试解析为其他 Lit 类型
    if let Ok(lit) = input.parse::<Lit>() {
        match lit {
            Lit::Str(lit_str) => {
                let str_value = lit_str.value();
                // 尝试直接解析字符串内容为 JSON
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
fn parse_value_string(s: &str) -> serde_json::Value {
    // 尝试直接作为 JSON 解析
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(s) {
        return val;
    }

    // 尝试作为字符串字面量（去掉引号）
    if let Some(inner) = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return serde_json::Value::String(inner.to_string());
    }

    // 尝试作为整数
    if let Ok(val) = s.parse::<i64>() {
        return serde_json::json!(val);
    }

    // 尝试作为浮点数
    if let Ok(val) = s.parse::<f64>() {
        return serde_json::json!(val);
    }

    // 尝试作为布尔值
    match s {
        "true" => return serde_json::json!(true),
        "false" => return serde_json::json!(false),
        _ => {}
    }

    // 默认作为字符串
    serde_json::Value::String(s.to_string())
}

/// 从输入中解析字面量为 f64
fn parse_lit_to_f64(input: syn::parse::ParseStream) -> syn::Result<Option<f64>> {
    // 尝试直接解析为 f64 字面量
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
fn parse_lit_to_usize(input: syn::parse::ParseStream) -> syn::Result<Option<usize>> {
    // 尝试直接解析为 usize 字面量
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
fn parse_lit_to_string(input: syn::parse::ParseStream) -> syn::Result<Option<String>> {
    if input.peek(LitStr) {
        let lit_str: LitStr = input.parse()?;
        return Ok(Some(lit_str.value()));
    }
    Ok(None)
}

/// 全局类型 schema 缓存（使用 LazyLock + Mutex 实现线程安全）
static TYPE_SCHEMA_CACHE: LazyLock<Mutex<BTreeMap<String, JsonSchema>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// `#[tool]` 宏入口
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    // 尝试解析为 impl 块
    if let Ok(impl_item) = syn::parse::<ItemImpl>(item.clone()) {
        // 是 impl 块，处理 impl 块级别的宏
        let attr_args = parse_macro_input!(attr as ToolAttributes);
        generate_for_impl(impl_item, attr_args).into()
    } else {
        // 不是 impl 块（可能是方法或参数），保留原样
        // 方法级别的 #[tool(...)] 由 extract_tool_info 处理
        // 参数级别的 #[tool(...)] 由 extract_params 中的 parse_param_tool_attrs 处理
        item
    }
}

/// `#[tool_type]` 宏入口 - 用于注册自定义类型的 schema
pub fn tool_type(attr: TokenStream, item: TokenStream) -> TokenStream {
    let struct_name = if let Ok(struct_item) = syn::parse::<ItemStruct>(item.clone()) {
        struct_item.ident.to_string()
    } else {
        return item;
    };

    // 解析属性中的 schema 定义
    if let Ok(schema_attrs) = syn::parse::<ToolTypeAttrs>(attr) {
        let schema = schema_attrs.to_json_schema();

        // 缓存 schema
        if let Ok(mut cache) = TYPE_SCHEMA_CACHE.lock() {
            cache.insert(struct_name, schema);
        }
    }

    // 保留原 struct 定义
    item
}

/// `#[tool_type]` 属性参数
#[allow(dead_code)]
struct ToolTypeAttrs {
    name: String,
    properties: Vec<(String, String)>, // (name, type)
    required: Vec<String>,
}

impl Parse for ToolTypeAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut properties = Vec::new();
        let mut required = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<token::Eq>()?;

            match key.to_string().as_str() {
                "name" => {
                    let value: LitStr = input.parse()?;
                    name = Some(value.value());
                }
                "properties" => {
                    // 解析 properties = "field1: string, field2: integer"
                    let value: LitStr = input.parse()?;
                    for prop in value.value().split(',') {
                        let parts: Vec<&str> = prop.trim().split(':').collect();
                        if parts.len() == 2 {
                            properties
                                .push((parts[0].trim().to_string(), parts[1].trim().to_string()));
                        }
                    }
                }
                "required" => {
                    // 解析 required = "field1, field2"
                    let value: LitStr = input.parse()?;
                    required = value
                        .value()
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                }
                _ => {
                    let value: LitStr = input.parse()?;
                    // 忽略未知属性
                    let _ = value;
                }
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        Ok(ToolTypeAttrs {
            name: name.unwrap_or_default(),
            properties,
            required,
        })
    }
}

impl ToolTypeAttrs {
    fn to_json_schema(&self) -> JsonSchema {
        let properties: BTreeMap<String, JsonSchema> = self
            .properties
            .iter()
            .map(|(name, ty)| {
                let schema = match ty.as_str() {
                    "string" => JsonSchema::string(None, None),
                    "integer" => JsonSchema::integer(None),
                    "number" => JsonSchema::number(None),
                    "boolean" => JsonSchema::boolean(None),
                    "array" => JsonSchema::Array {
                        ty: "array".to_string(),
                        items: Box::new(JsonSchema::Any {
                            description: None,
                            default: None,
                            deprecated: None,
                        }),
                        description: None,
                        prefix_items: None,
                        min_items: None,
                        max_items: None,
                        example: None,
                        default: None,
                        deprecated: None,
                        enum_values: None,
                    },
                    "object" => JsonSchema::Object {
                        ty: "object".to_string(),
                        properties: BTreeMap::new(),
                        required: vec![],
                        description: None,
                        additional_properties: None,
                        default: None,
                        deprecated: None,
                        tags: Vec::new(),
                        returns: None,
                        replaced_by: None,
                        context: None,
                        deprecated_note: None,
                    },
                    _ => JsonSchema::Any {
                        description: None,
                        default: None,
                        deprecated: None,
                    },
                };
                (name.clone(), schema)
            })
            .collect();

        JsonSchema::Object {
            ty: "object".to_string(),
            properties,
            required: self.required.clone(),
            description: None,
            additional_properties: None,
            default: None,
            deprecated: None,
            tags: Vec::new(),
            returns: None,
            replaced_by: None,
            context: None,
            deprecated_note: None,
        }
    }
}

/// impl 块级别的工具属性
#[derive(Default)]
struct ToolAttributes {
    #[allow(dead_code)]
    name: Option<String>,
    #[allow(dead_code)]
    description: Option<String>,
}

impl Parse for ToolAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<token::Eq>()?;

            let value: LitStr = input.parse()?;
            match key.to_string().as_str() {
                "name" => name = Some(value.value()),
                "desc" | "description" => description = Some(value.value()),
                _ => {}
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        Ok(ToolAttributes { name, description })
    }
}

/// 方法级别的工具属性
#[derive(Default)]
struct MethodToolAttrs {
    name: Option<String>,
    desc: Option<String>,
    skip: bool,
    deprecated: bool,
    replaced_by: Option<String>,
    deprecated_note: Option<String>,  // 新增：废弃说明
    deprecated_since: Option<String>, // 新增：废弃版本
    remove_in: Option<String>,        // 新增：移除版本
    version: Option<String>,          // 新增：版本
    visible: bool,
    tags: Vec<String>,
    group: Option<String>, // 新增：工具分组
    return_description: Option<String>,
    context: Option<String>,
    example_input: Option<serde_json::Value>, // 改为支持任意 JSON 值
    param_order: Option<Vec<String>>,
    hidden_params: Vec<String>,
    example_output: Option<String>,
    #[allow(dead_code)] // category 在解析后添加到 tags，不需要存储
    category: Option<String>,
    alias: Vec<String>,         // 别名列表
    allow: Vec<String>,         // 允许抑制的警告列表
    cache: Option<String>,      // 缓存配置（如 "ttl=60" 或 "key=user_id"）
    rate_limit: Option<String>, // 限流配置（如 "10/min" 或 "100/hour"）
    // 参数级别的验证属性（如 one_of_role, pattern_email 等）
    param_validations: Vec<(String, ParamToolAttrs)>,
}

impl Parse for MethodToolAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Ident) {
            let key: Ident = input.fork().parse()?;
            if key == "skip" {
                input.parse::<Ident>()?;
                if input.peek(token::Comma) {
                    input.parse::<token::Comma>()?;
                }
                return Ok(MethodToolAttrs {
                    name: None,
                    desc: None,
                    skip: true,
                    deprecated: false,
                    replaced_by: None,
                    deprecated_note: None,
                    deprecated_since: None,
                    remove_in: None,
                    version: None,
                    visible: true,
                    tags: Vec::new(),
                    group: None,
                    return_description: None,
                    context: None,
                    example_input: None,
                    param_order: None,
                    hidden_params: Vec::new(),
                    example_output: None,
                    category: None,
                    alias: Vec::new(),
                    allow: Vec::new(),
                    cache: None,
                    rate_limit: None,
                    param_validations: Vec::new(),
                });
            }
        }

        let mut name = None;
        let mut desc = None;
        let mut deprecated = false;
        let mut replaced_by = None;
        let mut deprecated_note = None;
        let mut deprecated_since = None;
        let mut remove_in = None;
        let mut version = None;
        let mut visible = true;
        let mut tags = Vec::new();
        let mut group = None;
        let mut return_description = None;
        let mut context = None;
        let mut example_input: Option<serde_json::Value> = None; // 改为支持任意 JSON 值
        let mut param_order: Option<Vec<String>> = None;
        let mut hidden_params = Vec::new();
        let mut example_output = None;
        let mut category: Option<String> = None;
        let mut alias = Vec::new();
        let mut allow = Vec::new();
        let mut cache: Option<String> = None;
        let mut rate_limit: Option<String> = None;
        let mut param_validations: Vec<(String, ParamToolAttrs)> = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            match key.to_string().as_str() {
                "deprecated" => {
                    // 处理 deprecated = true 或 deprecated（不带值）
                    if input.peek(token::Eq) {
                        input.parse::<token::Eq>()?;
                        // 尝试解析布尔值
                        if let Ok(lit_bool) = input.parse::<syn::LitBool>() {
                            deprecated = lit_bool.value;
                        } else {
                            deprecated = true;
                        }
                    } else {
                        deprecated = true;
                    }
                    // 消耗逗号
                    if input.peek(token::Comma) {
                        let _ = input.parse::<token::Comma>();
                    }
                }
                "replaced_by" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    replaced_by = Some(value.value());
                }
                "deprecated_note" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    deprecated_note = Some(value.value());
                }
                "deprecated_since" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    deprecated_since = Some(value.value());
                }
                "remove_in" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    remove_in = Some(value.value());
                }
                "version" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    version = Some(value.value());
                }
                "group" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    group = Some(value.value());
                }
                "visible" => {
                    input.parse::<token::Eq>()?;
                    // 尝试直接解析下一个 token
                    // 在 proc-macro 中，false 可能被解析为 Ident 或 Lit::Bool
                    // 尝试 1: 解析为 Ident
                    if let Ok(ident) = input.parse::<Ident>() {
                        visible = ident != "false";
                    }
                    // 尝试 2: 如果没有解析成功，尝试解析为 LitStr
                    else if input.peek(LitStr) {
                        let value: LitStr = input.parse()?;
                        visible = value.value().to_lowercase() != "false";
                    }
                    // 尝试 3: 如果没有解析成功，尝试解析为 Lit::Bool
                    else if let Ok(Lit::Bool(lit_bool)) = input.parse::<Lit>() {
                        visible = lit_bool.value;
                    }
                }
                "tags" => {
                    input.parse::<token::Eq>()?;
                    // 解析 tags = ["tag1", "tag2"]
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let tag: LitStr = content.parse()?;
                        tags.push(tag.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "return_description" | "returns" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    return_description = Some(value.value());
                }
                "context" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    context = Some(value.value());
                }
                "example_input" | "example" => {
                    input.parse::<token::Eq>()?;
                    // 支持字符串或任意字面量
                    example_input = parse_json_value(input)?;
                }
                "param_order" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    let mut order = Vec::new();
                    while !content.is_empty() {
                        let name: LitStr = content.parse()?;
                        order.push(name.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                    param_order = Some(order);
                }
                "hidden_params" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let name: LitStr = content.parse()?;
                        hidden_params.push(name.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "example_output" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    example_output = Some(value.value());
                }
                "category" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    category = Some(value.value());
                }
                "alias" => {
                    input.parse::<token::Eq>()?;
                    // 解析 alias = ["alias1", "alias2"]
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let alias_name: LitStr = content.parse()?;
                        alias.push(alias_name.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "allow" => {
                    input.parse::<token::Eq>()?;
                    // 解析 allow = ["deprecated_missing_replaced_by", "option_no_default"]
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let warning: LitStr = content.parse()?;
                        allow.push(warning.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "cache" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    cache = Some(value.value());
                }
                "rate_limit" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    rate_limit = Some(value.value());
                }
                _ => {
                    // 检查是否是已知的验证属性前缀（注意：更长的前缀必须放在前面）
                    let key_str = key.to_string();
                    let validation_prefixes = [
                        "enum_values_",
                        "min_length_",
                        "max_length_",
                        "min_items_",
                        "max_items_",
                        "multiple_of_",
                        "validate_msg_",
                        "default_",
                        "example_",
                        "one_of_",
                        "pattern_",
                        "min_",
                        "max_",
                    ];

                    let is_validation_attr = validation_prefixes
                        .iter()
                        .any(|prefix| key_str.starts_with(prefix));

                    if is_validation_attr {
                        // 处理参数级别的验证属性
                        for prefix in &validation_prefixes {
                            if key_str.starts_with(prefix) {
                                let param_name = key_str.strip_prefix(prefix).unwrap();
                                // 查找或创建该参数的验证属性
                                let existing_idx =
                                    param_validations.iter().position(|(n, _)| n == param_name);
                                let mut param_attrs = if let Some(idx) = existing_idx {
                                    param_validations.remove(idx).1
                                } else {
                                    ParamToolAttrs::default()
                                };

                                input.parse::<token::Eq>()?;

                                // 根据前缀设置对应的字段
                                match *prefix {
                                    "one_of_" => {
                                        // 解析数组值
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
                                        param_attrs.one_of = Some(values.clone());
                                    }
                                    "enum_values_" => {
                                        // 解析数组值
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
                                        param_attrs.enum_values = Some(values.clone());
                                    }
                                    "pattern_" => {
                                        param_attrs.pattern = parse_lit_to_string(input)?;
                                    }
                                    "min_" => {
                                        param_attrs.min = parse_lit_to_f64(input)?;
                                    }
                                    "max_" => {
                                        param_attrs.max = parse_lit_to_f64(input)?;
                                    }
                                    "min_length_" => {
                                        param_attrs.min_length = parse_lit_to_usize(input)?;
                                    }
                                    "max_length_" => {
                                        param_attrs.max_length = parse_lit_to_usize(input)?;
                                    }
                                    "min_items_" => {
                                        param_attrs.min_items = parse_lit_to_usize(input)?;
                                    }
                                    "max_items_" => {
                                        param_attrs.max_items = parse_lit_to_usize(input)?;
                                    }
                                    "multiple_of_" => {
                                        param_attrs.multiple_of = parse_lit_to_f64(input)?;
                                    }
                                    "validate_msg_" => {
                                        param_attrs.validate_msg = parse_lit_to_string(input)?;
                                    }
                                    "validate_msg_zh_" => {
                                        param_attrs.validate_msg_zh = parse_lit_to_string(input)?;
                                    }
                                    "validate_msg_en_" => {
                                        param_attrs.validate_msg_en = parse_lit_to_string(input)?;
                                    }
                                    "default_" => {
                                        param_attrs.default = parse_json_value(input)?;
                                    }
                                    "example_" => {
                                        param_attrs.example = parse_json_value(input)?;
                                    }
                                    _ => {}
                                }

                                param_validations.push((param_name.to_string(), param_attrs));
                                break;
                            }
                        }
                    } else {
                        // 处理普通属性（name, desc 等）
                        input.parse::<token::Eq>()?;
                        let value: LitStr = input.parse()?;
                        match key.to_string().as_str() {
                            "name" => name = Some(value.value()),
                            "desc" | "description" => desc = Some(value.value()),
                            _ => {}
                        }
                    }
                }
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        // category 添加到 tags
        if let Some(cat) = category {
            tags.push(cat);
        }

        Ok(MethodToolAttrs {
            name,
            desc,
            skip: false,
            deprecated,
            replaced_by,
            deprecated_note,
            deprecated_since,
            remove_in,
            version,
            visible,
            tags,
            group,
            return_description,
            context,
            example_input,
            param_order,
            hidden_params,
            example_output,
            category: None,
            alias,
            allow,
            cache,
            rate_limit,
            param_validations,
        })
    }
}

/// impl 块级别的工具属性
fn generate_for_impl(mut impl_item: ItemImpl, _attrs: ToolAttributes) -> TokenStream2 {
    let _impl_type = &impl_item.self_ty;

    let tool_methods = collect_tool_methods(&impl_item);

    if tool_methods.is_empty() {
        return quote! { #impl_item };
    }

    // 添加编译时警告（支持 #[tool(allow = [...])] 抑制）
    for tool in &tool_methods {
        // 警告 1: deprecated 方法没有指定 replaced_by
        if tool.deprecated
            && tool.replaced_by.is_none()
            && !tool
                .allow
                .contains(&"deprecated_missing_replaced_by".to_string())
        {
            eprintln!(
                "[tokitai] warning: method `{}` is marked deprecated without replaced_by\n\
                 💡 Suggestion: #[tool(deprecated, replaced_by = \"new_method\")]",
                tool.name
            );
        }

        // 警告 2: Option 类型参数没有默认值或示例
        for param in &tool.params {
            if param.is_option
                && param.default.is_none()
                && param.example.is_none()
                && !tool.allow.contains(&"option_no_default".to_string())
            {
                // 使用 schema_name（去掉 `_` 前缀）用于显示
                let display_name = &param.schema_name;
                eprintln!(
                    "[tokitai] warning: parameter `{}` is optional (Option type) without default or example\n\
                     → AI may not know this parameter can be omitted, which may cause call failures\n\
                     \n\
                     Suggested fixes (choose one):\n\
                     1. #[tool(default_{} = \"null\")]      # Add default value\n\
                     2. #[tool(example_{} = \"null\")]      # Add example\n\
                     3. Make it required: `{}: Option<T>` → `{}: T`",
                    display_name, display_name, display_name,
                    display_name, display_name
                );
            }
        }

        // 警告 3: 有 context = "async" 但方法不是 async
        if tool.context.as_deref() == Some("async")
            && !tool.is_async
            && !tool.allow.contains(&"context_async_mismatch".to_string())
        {
            eprintln!(
                "[tokitai] warning: method `{}` is marked context = \"async\" but is not an async method\n\
                 💡 Suggestion: remove context attribute or change method to async",
                tool.name
            );
        }
    }

    let impl_type = &impl_item.self_ty;
    let tool_def_consts = generate_tool_def_consts(&tool_methods);
    let all_tool_defs = generate_all_tool_defs_array(&tool_methods, impl_type);
    let call_tool_methods = generate_call_tool_method(&tool_methods);
    let helper_methods = generate_helper_methods(&tool_methods);

    let mut new_items: Vec<ImplItem> = impl_item.items.clone();

    // 将 static 定义添加到 impl 块内部
    for static_def in &tool_def_consts {
        // 将 static 定义解析为 ImplItem
        let static_item: ImplItem = syn::parse2(quote! {
            #[allow(dead_code)]
            #static_def
        }).unwrap_or_else(|e| {
            eprintln!("Failed to parse static definition: {}", e);
            syn::parse_quote! { fn __parse_error() { compile_error!("Failed to parse static definition"); } }
        });
        new_items.push(static_item);
    }

    let all_tool_defs_tokens = &all_tool_defs;
    // 使用 LazyLock 支持运行时配置覆盖
    // 在首次访问时应用配置

    // 生成工具定义获取函数（始终启用，支持配置覆盖）
    let get_tool_definitions_fn: ImplItem = parse_quote! {
        /// 所有工具定义（运行时初始化，支持配置覆盖）
        ///
        /// # 注意
        /// 此函数使用 `LazyLock` 进行延迟初始化。在初始化过程中会访问
        /// `GLOBAL_CONFIG_REGISTRY`，如果配置注册表也在 LazyLock 中初始化，
        /// 可能存在死锁风险。当前实现已确保初始化顺序安全。
        fn __get_tool_definitions() -> &'static [::tokitai::ToolDefinition] {
            static TOOLS: ::std::sync::LazyLock<::std::vec::Vec<::tokitai::ToolDefinition>> = ::std::sync::LazyLock::new(|| {
                // 编译期生成的原始定义（克隆静态定义）
                let mut defs = ::std::vec::Vec::from([#(#all_tool_defs_tokens.clone()),*]);

                // 应用运行时配置覆盖
                // 注意：GLOBAL_CONFIG_REGISTRY 也是 LazyLock，但在 TOOLS 之前初始化
                for def in &mut defs {
                    let configs = ::tokitai::GLOBAL_CONFIG_REGISTRY.get(&def.name);
                    if !configs.is_empty() {
                        def.apply_configs(&configs);
                    }
                }

                defs
            });

            &TOOLS
        }
    };
    new_items.push(get_tool_definitions_fn);

    for method in call_tool_methods {
        new_items.push(parse_quote! { #method });
    }

    for helper in helper_methods {
        new_items.push(parse_quote! { #helper });
    }

    // 添加 configure_tool 方法用于配置宏
    new_items.push(parse_quote! {
        /// 配置工具属性（运行时覆盖）
        ///
        /// 此方法由 `tokitai!` 配置宏调用，用于在运行时覆盖工具定义。
        ///
        /// # 注意
        ///
        /// 此方法需要在首次访问工具定义前调用，否则配置可能不会生效。
        pub fn configure_tool(_tool_name: &str, _configs: &[::tokitai::ToolConfig]) {
            // 注册配置到全局注册表
            ::tokitai::GLOBAL_CONFIG_REGISTRY.configure(_tool_name, _configs);

            // 触发 LazyLock 初始化（通过访问一次来确保配置应用）
            // 这确保配置在首次访问 tool_definitions() 之前被应用
            let _ = Self::__get_tool_definitions();
        }
    });

    impl_item.items = new_items;

    // 获取类型名称用于 ToolProvider 实现
    let impl_type = &impl_item.self_ty;

    quote! {
        #impl_item

        // 实现 ToolProvider trait
        impl ::tokitai::ToolProvider for #impl_type {
            fn tool_definitions() -> &'static [::tokitai::ToolDefinition] {
                Self::__get_tool_definitions()
            }
        }

        // 实现 ToolCaller trait
        // 注意：这里直接委托给宏生成的 call_tool 方法
        // 由于 serde_types::Value 就是 serde_json::Value，类型完全兼容
        impl ::tokitai_core::ToolCaller for #impl_type {
            fn call_tool(&self, name: &str, args: &::tokitai_core::serde_types::Value) -> Result<::tokitai_core::serde_types::Value, ::tokitai_core::ToolError> {
                // 直接调用宏生成的 call_tool 方法（类型完全兼容）
                <#impl_type>::call_tool(self, name, args)
            }
        }
    }
}

/// 收集所有被标记为工具的方法
fn collect_tool_methods(impl_item: &ItemImpl) -> Vec<ToolMethodInfo> {
    let mut tools = Vec::new();

    for item in &impl_item.items {
        if let ImplItem::Fn(fn_item) = item {
            if !matches!(fn_item.vis, Visibility::Public(_)) {
                continue;
            }

            if let Some(tool_info) = extract_tool_info(fn_item) {
                tools.push(tool_info);
            }
        }
    }

    tools
}

/// 提取工具方法信息
fn extract_tool_info(fn_item: &ImplItemFn) -> Option<ToolMethodInfo> {
    let method_name = fn_item.sig.ident.to_string();

    if method_name.starts_with("__") {
        return None;
    }

    if !fn_item.sig.generics.params.is_empty() {
        return Some(ToolMethodInfo {
            name: method_name.clone(),
            tool_name: method_name.clone(),
            description: String::new(),
            params: vec![],
            is_async: false,
            is_result: false,
            is_generic: true,
            deprecated: false,
            replaced_by: None,
            deprecated_note: None,
            deprecated_since: None,
            remove_in: None,
            version: None,
            visible: true,
            tags: Vec::new(),
            group: None,
            return_description: None,
            context: None,
            example_input: None,
            param_order: None,
            hidden_params: Vec::new(),
            example_output: None,
            return_type: fn_item.sig.output.clone(),
            doc: None,
            alias: Vec::new(),
            allow: Vec::new(),
            cache: None,
            rate_limit: None,
            param_validations: Vec::new(),
        });
    }

    let mut custom_name = None;
    let mut custom_desc = None;
    let mut should_skip = false;
    let mut is_deprecated = false;
    let mut replaced_by = None;
    let mut deprecated_note = None;
    let mut deprecated_since = None;
    let mut remove_in = None;
    let mut version = None;
    let mut is_visible = true;
    let mut tool_tags = Vec::new();
    let mut group = None;
    let mut return_description = None;
    let mut context = None;
    let mut example_input: Option<serde_json::Value> = None; // 改为支持任意 JSON 值
    let mut param_order: Option<Vec<String>> = None;
    let mut hidden_params = Vec::new();
    let mut example_output = None;
    let mut alias = Vec::new();
    let mut allow = Vec::new();
    let mut cache: Option<String> = None;
    let mut rate_limit: Option<String> = None;
    let mut param_validations: Vec<(String, ParamToolAttrs)> = Vec::new();

    for attr in &fn_item.attrs {
        if attr.path().is_ident("tool") {
            if let Ok(args) = attr.parse_args::<MethodToolAttrs>() {
                if args.skip {
                    should_skip = true;
                    break;
                }
                custom_name = args.name;
                custom_desc = args.desc;
                is_deprecated = args.deprecated;
                replaced_by = args.replaced_by;
                deprecated_note = args.deprecated_note;
                deprecated_since = args.deprecated_since;
                remove_in = args.remove_in;
                version = args.version;
                is_visible = args.visible;
                tool_tags = args.tags;
                group = args.group;
                return_description = args.return_description;
                context = args.context;
                example_input = args.example_input;
                param_order = args.param_order;
                hidden_params = args.hidden_params;
                example_output = args.example_output;
                alias = args.alias;
                allow = args.allow;
                cache = args.cache;
                rate_limit = args.rate_limit;
                param_validations = args.param_validations;
            }
        }
    }

    if should_skip {
        return None;
    }

    // 如果 visible = false，跳过该工具（不添加到 TOOL_DEFINITIONS）
    if !is_visible {
        return None;
    }

    let tool_name = custom_name.unwrap_or_else(|| method_name.clone());

    let description = custom_desc
        .or_else(|| extract_doc_comment(&fn_item.attrs))
        .unwrap_or_else(|| format!("调用 {} 方法", method_name));

    let params = extract_params(
        &fn_item.sig.inputs,
        &fn_item.attrs,
        &hidden_params,
        &param_validations,
    );
    let is_async = fn_item.sig.asyncness.is_some();
    let is_result = is_result_type(&fn_item.sig.output);

    Some(ToolMethodInfo {
        name: method_name,
        tool_name,
        description,
        params,
        is_async,
        is_result,
        is_generic: false,
        deprecated: is_deprecated,
        replaced_by,
        deprecated_note,
        deprecated_since,
        remove_in,
        version,
        visible: is_visible,
        tags: tool_tags,
        group,
        return_description,
        context,
        example_input,
        param_order,
        hidden_params,
        example_output,
        return_type: fn_item.sig.output.clone(),
        doc: None,
        alias,
        allow,
        cache,
        rate_limit,
        param_validations,
    })
}

/// 工具方法信息
#[allow(dead_code)]
struct ToolMethodInfo {
    name: String,
    tool_name: String,
    description: String,
    params: Vec<ParamInfo>,
    is_async: bool,
    is_result: bool,
    is_generic: bool,
    deprecated: bool,
    replaced_by: Option<String>,
    deprecated_note: Option<String>,  // 新增：废弃说明
    deprecated_since: Option<String>, // 新增：废弃版本
    remove_in: Option<String>,        // 新增：移除版本
    version: Option<String>,          // 新增：版本
    visible: bool,
    tags: Vec<String>,
    group: Option<String>, // 新增：工具分组
    return_description: Option<String>,
    context: Option<String>,
    example_input: Option<serde_json::Value>, // 改为支持任意 JSON 值
    param_order: Option<Vec<String>>,
    hidden_params: Vec<String>,
    example_output: Option<String>,
    return_type: ReturnType,
    doc: Option<String>,
    alias: Vec<String>,                               // 别名列表
    allow: Vec<String>,                               // 允许抑制的警告列表
    cache: Option<String>,                            // 缓存配置
    rate_limit: Option<String>,                       // 限流配置
    param_validations: Vec<(String, ParamToolAttrs)>, // 参数级别的验证属性
}

/// 参数信息
#[allow(dead_code)] // 部分字段用于未来扩展
struct ParamInfo {
    name: Ident,         // 原始参数名（用于方法调用，如 `_name`）
    schema_name: String, // Schema 中的名称（去掉 `_` 前缀，如 `name`）
    ty: Type,
    description: Option<String>,
    is_option: bool,
    is_required: bool,                  // 显式标记为必需（覆盖 Option 类型）
    example: Option<serde_json::Value>, // 改为支持任意 JSON 值
    default: Option<serde_json::Value>, // 改为支持任意 JSON 值
    validate: Option<String>,           // 验证表达式
    transform: Option<String>,          // 转换表达式
    // JSON Schema 验证属性
    one_of: Option<Vec<String>>,
    enum_values: Option<Vec<serde_json::Value>>,
    pattern: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    min_length: Option<usize>,
    max_length: Option<usize>,
    min_items: Option<usize>,
    max_items: Option<usize>,
    multiple_of: Option<f64>,
    validate_msg: Option<String>,
    validate_msg_zh: Option<String>,
    validate_msg_en: Option<String>,
}

/// JSON Schema 中间表示 AST
/// 使用 derive(Serialize) 直接序列化为 JSON
#[derive(Serialize, Clone)]
#[serde(untagged)]
enum JsonSchema {
    /// 基本类型
    Basic {
        #[serde(rename = "type")]
        ty: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        format: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        example: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<bool>,
        // JSON Schema 验证属性
        #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
        enum_values: Option<Vec<serde_json::Value>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum: Option<f64>,
        #[serde(rename = "minLength", skip_serializing_if = "Option::is_none")]
        min_length: Option<usize>,
        #[serde(rename = "maxLength", skip_serializing_if = "Option::is_none")]
        max_length: Option<usize>,
        #[serde(rename = "multipleOf", skip_serializing_if = "Option::is_none")]
        multiple_of: Option<f64>,
    },
    /// 数组类型
    Array {
        #[serde(rename = "type")]
        ty: String,
        items: Box<JsonSchema>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(rename = "prefixItems", skip_serializing_if = "Option::is_none")]
        prefix_items: Option<Vec<JsonSchema>>,
        #[serde(rename = "minItems", skip_serializing_if = "Option::is_none")]
        min_items: Option<usize>,
        #[serde(rename = "maxItems", skip_serializing_if = "Option::is_none")]
        max_items: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        example: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<bool>,
        // JSON Schema 验证属性
        #[serde(skip_serializing_if = "Option::is_none")]
        enum_values: Option<Vec<serde_json::Value>>,
    },
    /// 对象类型
    Object {
        #[serde(rename = "type")]
        ty: String,
        properties: BTreeMap<String, JsonSchema>,
        required: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(
            rename = "additionalProperties",
            skip_serializing_if = "Option::is_none"
        )]
        additional_properties: Option<Box<JsonSchema>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<bool>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        returns: Option<Box<JsonSchema>>,
        #[serde(rename = "x-replaced-by", skip_serializing_if = "Option::is_none")]
        replaced_by: Option<String>,
        #[serde(rename = "x-context", skip_serializing_if = "Option::is_none")]
        context: Option<String>,
        #[serde(rename = "x-deprecated-note", skip_serializing_if = "Option::is_none")]
        deprecated_note: Option<String>,
    },
    /// 可空类型（Option<T>）- 使用 anyOf
    Nullable {
        #[serde(rename = "anyOf")]
        any_of: Vec<JsonSchema>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<bool>,
    },
    /// 任意 JSON 值
    Any {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        deprecated: Option<bool>,
    },
}

impl JsonSchema {
    /// 创建字符串 schema
    fn string(description: Option<String>, format: Option<String>) -> Self {
        JsonSchema::Basic {
            ty: "string".to_string(),
            description,
            format,
            example: None,
            default: None,
            deprecated: None,
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            multiple_of: None,
        }
    }

    /// 创建字符串 schema with example
    #[allow(dead_code)]
    fn string_with_example(
        description: Option<String>,
        format: Option<String>,
        example: Option<String>,
    ) -> Self {
        JsonSchema::Basic {
            ty: "string".to_string(),
            description,
            format,
            example,
            default: None,
            deprecated: None,
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            multiple_of: None,
        }
    }

    /// 创建字符串 schema with example and default
    fn string_with_example_and_default(
        description: Option<String>,
        format: Option<String>,
        example: Option<String>,
        default: Option<serde_json::Value>,
    ) -> Self {
        JsonSchema::Basic {
            ty: "string".to_string(),
            description,
            format,
            example,
            default,
            deprecated: None,
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            multiple_of: None,
        }
    }

    /// 创建整数 schema
    fn integer(description: Option<String>) -> Self {
        JsonSchema::Basic {
            ty: "integer".to_string(),
            description,
            format: None,
            example: None,
            default: None,
            deprecated: None,
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            multiple_of: None,
        }
    }

    /// 创建整数 schema with default
    fn integer_with_default(
        description: Option<String>,
        default: Option<serde_json::Value>,
    ) -> Self {
        JsonSchema::Basic {
            ty: "integer".to_string(),
            description,
            format: None,
            example: None,
            default,
            deprecated: None,
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            multiple_of: None,
        }
    }

    /// 创建数字 schema
    fn number(description: Option<String>) -> Self {
        JsonSchema::Basic {
            ty: "number".to_string(),
            description,
            format: None,
            example: None,
            default: None,
            deprecated: None,
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            multiple_of: None,
        }
    }

    /// 创建数字 schema with default
    fn number_with_default(
        description: Option<String>,
        default: Option<serde_json::Value>,
    ) -> Self {
        JsonSchema::Basic {
            ty: "number".to_string(),
            description,
            format: None,
            example: None,
            default,
            deprecated: None,
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            multiple_of: None,
        }
    }

    /// 创建布尔 schema
    fn boolean(description: Option<String>) -> Self {
        JsonSchema::Basic {
            ty: "boolean".to_string(),
            description,
            format: None,
            example: None,
            default: None,
            deprecated: None,
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            multiple_of: None,
        }
    }

    /// 创建布尔 schema with default
    fn boolean_with_default(
        description: Option<String>,
        default: Option<serde_json::Value>,
    ) -> Self {
        JsonSchema::Basic {
            ty: "boolean".to_string(),
            description,
            format: None,
            example: None,
            default,
            deprecated: None,
            enum_values: None,
            pattern: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            multiple_of: None,
        }
    }

    /// 创建可空 schema（JSON Schema 标准格式）
    #[allow(dead_code)]
    fn nullable(inner: JsonSchema) -> Self {
        // 扁平化嵌套的 Option
        let flat_inner = flatten_option_schema(inner);
        JsonSchema::Nullable {
            any_of: vec![
                flat_inner,
                JsonSchema::Basic {
                    ty: "null".to_string(),
                    description: None,
                    format: None,
                    example: None,
                    default: None,
                    deprecated: None,
                    enum_values: None,
                    pattern: None,
                    minimum: None,
                    maximum: None,
                    min_length: None,
                    max_length: None,
                    multiple_of: None,
                },
            ],
            description: None,
            default: None,
            deprecated: None,
        }
    }

    /// 创建可空 schema with description and default（JSON Schema 标准格式）
    fn nullable_with_description_and_default(
        inner: JsonSchema,
        description: Option<String>,
        default: Option<serde_json::Value>,
    ) -> Self {
        // 扁平化嵌套的 Option
        let flat_inner = flatten_option_schema(inner);
        JsonSchema::Nullable {
            any_of: vec![
                flat_inner,
                JsonSchema::Basic {
                    ty: "null".to_string(),
                    description: None,
                    format: None,
                    example: None,
                    default: None,
                    deprecated: None,
                    enum_values: None,
                    pattern: None,
                    minimum: None,
                    maximum: None,
                    min_length: None,
                    max_length: None,
                    multiple_of: None,
                },
            ],
            description,
            default,
            deprecated: None,
        }
    }

    /// 转换为 JSON 字符串
    fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// 获取 description（如果存在）
    fn description(&self) -> Option<&String> {
        match self {
            JsonSchema::Basic { description, .. } => description.as_ref(),
            JsonSchema::Array { description, .. } => description.as_ref(),
            JsonSchema::Object { description, .. } => description.as_ref(),
            JsonSchema::Nullable { description, .. } => description.as_ref(),
            _ => None,
        }
    }

    /// 设置 description
    fn set_description(&mut self, desc: Option<String>) {
        match self {
            JsonSchema::Basic { description, .. } => *description = desc,
            JsonSchema::Array { description, .. } => *description = desc,
            JsonSchema::Object { description, .. } => *description = desc,
            JsonSchema::Nullable { description, .. } => *description = desc,
            _ => {}
        }
    }
}

/// 扁平化嵌套的 Option schema
/// 保留 deprecated 和 description 信息到扁平化后的 schema
fn flatten_option_schema(schema: JsonSchema) -> JsonSchema {
    match schema {
        JsonSchema::Nullable {
            any_of,
            deprecated,
            description,
            ..
        } => {
            // 提取内部的非 null 类型
            let inner = any_of
                .into_iter()
                .find(|s| !matches!(s, JsonSchema::Any { .. }))
                .unwrap_or(JsonSchema::Any {
                    description: None,
                    default: None,
                    deprecated: None,
                });
            // 递归扁平化
            let mut flat = flatten_option_schema(inner);
            // 保留 deprecated 和 description 信息到扁平化后的 schema
            match &mut flat {
                JsonSchema::Basic {
                    deprecated: d,
                    description: desc,
                    ..
                }
                | JsonSchema::Array {
                    deprecated: d,
                    description: desc,
                    ..
                }
                | JsonSchema::Object {
                    deprecated: d,
                    description: desc,
                    ..
                }
                | JsonSchema::Nullable {
                    deprecated: d,
                    description: desc,
                    ..
                }
                | JsonSchema::Any {
                    deprecated: d,
                    description: desc,
                    ..
                } => {
                    *d = deprecated;
                    // 如果内部类型没有 description，使用外层的 description
                    if desc.is_none() && description.is_some() {
                        *desc = description;
                    }
                }
            }
            flat
        }
        _ => schema,
    }
}

/// 生成编译期工具定义函数
fn generate_tool_def_consts(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
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

        // 生成主工具定义函数
        let const_name = format_ident!("__TOOL_DEF_{}", tool.name.to_uppercase());
        let tool_name = &tool.tool_name;
        let description = &tool.description;

        let schema_json = generate_schema_json_with_deprecated_and_tags(
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

        // 构建 ToolDefinition，支持 version 字段
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

        // 为每个别名生成工具定义函数
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
fn generate_all_tool_defs_array(tools: &[ToolMethodInfo], impl_type: &Type) -> Vec<TokenStream2> {
    let mut defs = Vec::new();

    for tool in tools {
        // 添加主工具定义（调用函数）
        let const_name = format_ident!("__TOOL_DEF_{}", tool.name.to_uppercase());
        defs.push(quote! { #impl_type::#const_name() });

        // 添加别名定义（调用函数）
        for (i, _alias_name) in tool.alias.iter().enumerate() {
            let alias_const_name =
                format_ident!("__TOOL_DEF_ALIAS_{}_{}", tool.name.to_uppercase(), i);
            defs.push(quote! { #impl_type::#alias_const_name() });
        }
    }

    defs
}

/// 生成 call_tool 分发方法
fn generate_call_tool_method(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
    let mut methods = Vec::new();
    let has_async = tools.iter().any(|t| t.is_async);

    // 生成可用工具列表文档
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

            // 生成主名称和所有别名的匹配臂
            let mut arms = Vec::new();

            // 主名称
            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    self.#wrapper_name(args).await
                }
            });

            // 别名
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

            // 生成主名称和所有别名的匹配臂
            let mut arms = Vec::new();

            // 主名称
            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    self.#wrapper_name(args)
                }
            });

            // 别名
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

            // 生成主名称和所有别名的匹配臂
            let mut arms = Vec::new();

            // 主名称
            let tool_name = &tool.tool_name;
            arms.push(quote! {
                #tool_name => {
                    self.#wrapper_name_sync(args)
                }
            });

            // 别名
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

/// 生成参数解析辅助方法
fn generate_helper_methods(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
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
fn generate_wrapper_method_sync(tool: &ToolMethodInfo) -> TokenStream2 {
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

    // 生成验证代码
    let param_validations = params.iter().flat_map(|p| {
        let mut validations = Vec::new();
        let param_name = &p.name;
        let schema_name_str = &p.schema_name;

        // 1. 处理 @validate 自定义验证
        if let Some(validate_expr) = &p.validate {
            let validate_code = validate_expr.replace("value", &format!("{}", param_name));
            let validate_expr_tokens: Expr = match syn::parse_str(&validate_code) {
                Ok(expr) => expr,
                Err(e) => {
                    eprintln!("[tokitai] warning: failed to parse validation expression: {} - {}", validate_code, e);
                    return Vec::new();
                }
            };
            // 使用自定义错误消息或默认消息（支持多语言）
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

        // 2. 为 one_of 生成运行时验证（仅适用于 String 类型）
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

        // 3. 为 pattern 生成运行时验证（仅适用于 String 类型）
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

        // 4. 为 min 生成运行时验证（仅适用于数值类型）
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

        // 5. 为 max 生成运行时验证（仅适用于数值类型）
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

        // 6. 为 min_length 生成运行时验证（适用于 String 或 Vec）
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

        // 7. 为 max_length 生成运行时验证（适用于 String 或 Vec）
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

        // 8. 为 multiple_of 生成运行时验证（仅适用于数值类型）
        // 使用 quotient.round() 方法避免浮点数精度问题
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

    // 生成转换代码
    let param_transforms = params.iter().filter_map(|p| {
        if let Some(transform_expr) = &p.transform {
            let param_name = &p.name;
            let transform_code = transform_expr.replace("value", &format!("{}", param_name));
            // 将字符串解析为表达式
            let transform_expr_tokens: Expr = match syn::parse_str(&transform_code) {
                Ok(expr) => expr,
                Err(e) => {
                    eprintln!(
                        "[tokitai] warning: failed to parse transform expression: {} - {}",
                        transform_code, e
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
fn generate_wrapper_method(tool: &ToolMethodInfo, is_async: bool) -> TokenStream2 {
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

    // 生成验证代码
    let param_validations = params.iter().flat_map(|p| {
        let mut validations = Vec::new();
        let param_name = &p.name;
        let schema_name_str = &p.schema_name;

        // 1. 处理 @validate 自定义验证
        if let Some(validate_expr) = &p.validate {
            let validate_code = validate_expr.replace("value", &format!("{}", param_name));
            let validate_expr_tokens: Expr = match syn::parse_str(&validate_code) {
                Ok(expr) => expr,
                Err(e) => {
                    eprintln!("[tokitai] warning: failed to parse validation expression: {} - {}", validate_code, e);
                    return Vec::new();
                }
            };
            // 使用自定义错误消息或默认消息（支持多语言）
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

        // 2. 为 one_of 生成运行时验证（仅适用于 String 类型）
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

        // 3. 为 pattern 生成运行时验证（仅适用于 String 类型）
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

        // 4. 为 min 生成运行时验证（仅适用于数值类型）
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

        // 5. 为 max 生成运行时验证（仅适用于数值类型）
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

        // 6. 为 min_length 生成运行时验证（适用于 String 或 Vec）
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

        // 7. 为 max_length 生成运行时验证（适用于 String 或 Vec）
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

        // 8. 为 multiple_of 生成运行时验证（仅适用于数值类型）
        // 使用 quotient.round() 方法避免浮点数精度问题
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

    // 生成转换代码
    let param_transforms = params.iter().filter_map(|p| {
        if let Some(transform_expr) = &p.transform {
            let param_name = &p.name;
            let transform_code = transform_expr.replace("value", &format!("{}", param_name));
            // 将字符串解析为表达式
            let transform_expr_tokens: Expr = match syn::parse_str(&transform_code) {
                Ok(expr) => expr,
                Err(e) => {
                    eprintln!(
                        "[tokitai] warning: failed to parse transform expression: {} - {}",
                        transform_code, e
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

/// JSON Schema 生成配置（Builder 模式）
struct SchemaGenConfig<'a> {
    params: &'a [ParamInfo],
    deprecated: bool,
    replaced_by: Option<&'a str>,
    context: Option<&'a str>,
    tags: &'a [String],
    return_description: Option<&'a str>,
    example_input: Option<&'a serde_json::Value>,
    param_order: Option<&'a [String]>,
    example_output: Option<&'a str>,
    deprecated_note: Option<&'a str>,
    deprecated_since: Option<&'a str>,
    remove_in: Option<&'a str>,
    group: Option<&'a str>,
    cache: Option<&'a str>,
    rate_limit: Option<&'a str>,
}

impl<'a> SchemaGenConfig<'a> {
    fn new(params: &'a [ParamInfo]) -> Self {
        Self {
            params,
            deprecated: false,
            replaced_by: None,
            context: None,
            tags: &[],
            return_description: None,
            example_input: None,
            param_order: None,
            example_output: None,
            deprecated_note: None,
            deprecated_since: None,
            remove_in: None,
            group: None,
            cache: None,
            rate_limit: None,
        }
    }

    pub fn deprecated(mut self, val: bool) -> Self {
        self.deprecated = val;
        self
    }

    pub fn replaced_by(mut self, val: Option<&'a str>) -> Self {
        self.replaced_by = val;
        self
    }

    pub fn context(mut self, val: Option<&'a str>) -> Self {
        self.context = val;
        self
    }

    pub fn tags(mut self, val: &'a [String]) -> Self {
        self.tags = val;
        self
    }

    pub fn return_description(mut self, val: Option<&'a str>) -> Self {
        self.return_description = val;
        self
    }

    pub fn example_input(mut self, val: Option<&'a serde_json::Value>) -> Self {
        self.example_input = val;
        self
    }

    pub fn param_order(mut self, val: Option<&'a [String]>) -> Self {
        self.param_order = val;
        self
    }

    pub fn example_output(mut self, val: Option<&'a str>) -> Self {
        self.example_output = val;
        self
    }

    pub fn deprecated_note(mut self, val: Option<&'a str>) -> Self {
        self.deprecated_note = val;
        self
    }

    pub fn deprecated_since(mut self, val: Option<&'a str>) -> Self {
        self.deprecated_since = val;
        self
    }

    pub fn remove_in(mut self, val: Option<&'a str>) -> Self {
        self.remove_in = val;
        self
    }

    pub fn group(mut self, val: Option<&'a str>) -> Self {
        self.group = val;
        self
    }

    pub fn cache(mut self, val: Option<&'a str>) -> Self {
        self.cache = val;
        self
    }

    pub fn rate_limit(mut self, val: Option<&'a str>) -> Self {
        self.rate_limit = val;
        self
    }

    /// 构建配置对象（链式调用终点）
    ///
    /// 注意：此方法是可选的，因为所有 Builder 方法都返回 `Self`，
    /// 可以直接使用链式调用的最终结果。此方法主要用于明确链式调用的结束。
    #[allow(dead_code)] // 供未来使用或外部调用
    pub fn build(self) -> Self {
        self
    }
}

/// 生成 JSON Schema（支持 deprecated、tags、return_description、example_input、param_order、example_output、deprecated_note、deprecated_since、remove_in、group）
fn generate_schema_json_with_deprecated_and_tags(config: &SchemaGenConfig) -> String {
    let mut properties: BTreeMap<String, JsonSchema> = BTreeMap::new();
    let mut required = Vec::new();

    for p in config.params {
        let schema_name = p.schema_name.clone();
        let mut schema = generate_schema_for_type_with_default_and_example(
            &p.ty,
            p.description.clone(),
            p.example.as_ref(),
            p.default.as_ref(),
        );

        // 添加验证属性到 schema
        match &mut schema {
            JsonSchema::Basic {
                enum_values,
                pattern,
                minimum,
                maximum,
                min_length,
                max_length,
                multiple_of,
                ..
            } => {
                if p.one_of.is_some() {
                    // 将 one_of 转换为 enum_values（字符串数组）
                    let vals = p
                        .one_of
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect();
                    *enum_values = Some(vals);
                }
                if p.enum_values.is_some() {
                    *enum_values = p.enum_values.clone();
                }
                if p.pattern.is_some() {
                    *pattern = p.pattern.clone();
                }
                if p.min.is_some() {
                    *minimum = p.min;
                }
                if p.max.is_some() {
                    *maximum = p.max;
                }
                if p.min_length.is_some() {
                    *min_length = p.min_length;
                }
                if p.max_length.is_some() {
                    *max_length = p.max_length;
                }
                if p.multiple_of.is_some() {
                    *multiple_of = p.multiple_of;
                }
            }
            JsonSchema::Array {
                enum_values,
                min_items,
                max_items,
                ..
            } => {
                if p.one_of.is_some() {
                    let vals = p
                        .one_of
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect();
                    *enum_values = Some(vals);
                }
                if p.enum_values.is_some() {
                    *enum_values = p.enum_values.clone();
                }
                if p.min_items.is_some() {
                    *min_items = p.min_items;
                }
                if p.max_items.is_some() {
                    *max_items = p.max_items;
                }
            }
            _ => {}
        }

        // ✅ 如果 schema 没有 description，但参数有，则使用参数的
        if schema.description().is_none() && p.description.is_some() {
            schema.set_description(p.description.clone());
        }

        properties.insert(schema_name.clone(), schema);

        // 如果显式标记为 required 或者不是 Option 类型，则加入 required 列表
        if p.is_required || !p.is_option {
            required.push(schema_name);
        }
    }

    // 生成 returns schema
    let returns_schema = config.return_description.map(|desc| JsonSchema::Basic {
        ty: "string".to_string(),
        description: Some(desc.to_string()),
        format: None,
        example: config.example_output.map(|s| s.to_string()),
        default: None,
        deprecated: None,
        enum_values: None,
        pattern: None,
        minimum: None,
        maximum: None,
        min_length: None,
        max_length: None,
        multiple_of: None,
    });

    // 解析 example_input
    let examples = config.example_input.map(|val| vec![val.clone()]);

    let schema = JsonSchema::Object {
        ty: "object".to_string(),
        properties,
        required,
        description: None,
        additional_properties: None,
        default: None,
        deprecated: if config.deprecated { Some(true) } else { None },
        tags: config.tags.to_vec(),
        returns: returns_schema.map(Box::new),
        replaced_by: config.replaced_by.map(|s| s.to_string()),
        context: config.context.map(|s| s.to_string()),
        deprecated_note: config.deprecated_note.map(|s| s.to_string()),
    };

    // 如果有 examples、param_order、deprecated_since、remove_in、group、cache 或 rate_limit，需要手动添加到 JSON 输出
    let mut json_str = schema.to_json_string();
    let needs_update = examples.is_some()
        || config.param_order.is_some()
        || config.deprecated_since.is_some()
        || config.remove_in.is_some()
        || config.group.is_some()
        || config.cache.is_some()
        || config.rate_limit.is_some();

    if needs_update {
        if let Ok(mut json_obj) =
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&json_str)
        {
            if let Some(examples_val) = examples {
                json_obj.insert(
                    "examples".to_string(),
                    serde_json::to_value(examples_val).unwrap(),
                );
            }
            if let Some(order) = config.param_order {
                json_obj.insert(
                    "x-param-order".to_string(),
                    serde_json::to_value(order).unwrap(),
                );
            }
            if let Some(since) = config.deprecated_since {
                json_obj.insert(
                    "x-deprecated-since".to_string(),
                    serde_json::to_value(since).unwrap(),
                );
            }
            if let Some(remove) = config.remove_in {
                json_obj.insert(
                    "x-remove-in".to_string(),
                    serde_json::to_value(remove).unwrap(),
                );
            }
            if let Some(g) = config.group {
                json_obj.insert("x-group".to_string(), serde_json::to_value(g).unwrap());
            }
            if let Some(c) = config.cache {
                json_obj.insert("x-cache".to_string(), serde_json::to_value(c).unwrap());
            }
            if let Some(r) = config.rate_limit {
                json_obj.insert("x-rate-limit".to_string(), serde_json::to_value(r).unwrap());
            }
            json_str = serde_json::to_string(&json_obj).unwrap();
        }
    }

    json_str
}

/// 生成 JSON Schema（向后兼容的旧函数）
#[allow(dead_code)]
fn generate_schema_json(params: &[ParamInfo]) -> String {
    generate_schema_json_with_deprecated_and_tags(&SchemaGenConfig::new(params))
}

/// 为类型生成 JSON Schema（递归解析）
#[allow(dead_code)]
fn generate_schema_for_type(ty: &Type, description: Option<String>) -> JsonSchema {
    generate_schema_for_type_with_default_and_example(ty, description, None, None)
}

/// 为类型生成 JSON Schema（递归解析，支持 example）
#[allow(dead_code)]
fn generate_schema_for_type_with_example(
    ty: &Type,
    description: Option<String>,
    example: Option<&serde_json::Value>,
) -> JsonSchema {
    generate_schema_for_type_with_default_and_example(ty, description, example, None)
}

/// 为类型生成 JSON Schema（递归解析，支持 example 和 default）
fn generate_schema_for_type_with_default_and_example(
    ty: &Type,
    description: Option<String>,
    example: Option<&serde_json::Value>,
    default: Option<&serde_json::Value>,
) -> JsonSchema {
    let default_value = default.cloned();

    match ty {
        Type::Path(path) => {
            let ident = path
                .path
                .segments
                .first()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();

            match ident.as_str() {
                // 基本类型
                "String" => JsonSchema::string_with_example_and_default(
                    description,
                    None,
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                ),
                "str" => JsonSchema::string_with_example_and_default(
                    description,
                    Some("string".to_string()),
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                ),
                "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128"
                | "usize" | "isize" => JsonSchema::integer_with_default(description, default_value),
                "f32" | "f64" => JsonSchema::number_with_default(description, default_value),
                "bool" => JsonSchema::boolean_with_default(description, default_value),

                // Option<T> - 可空类型
                "Option" => {
                    if let Some(last_segment) = path.path.segments.last() {
                        if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                // 传递 description 到内部类型
                                let inner_schema =
                                    generate_schema_for_type_with_default_and_example(
                                        inner_ty,
                                        description.clone(), // 保留 description
                                        None,
                                        None,
                                    );
                                return JsonSchema::nullable_with_description_and_default(
                                    inner_schema,
                                    description,
                                    default_value,
                                );
                            }
                        }
                    }
                    JsonSchema::Any {
                        description,
                        default: default_value,
                        deprecated: None,
                    }
                }

                // Vec<T> - 数组类型
                "Vec" => {
                    if let Some(last_segment) = path.path.segments.last() {
                        if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                let items_schema =
                                    generate_schema_for_type_with_default_and_example(
                                        inner_ty, None, None, None,
                                    );
                                return JsonSchema::Array {
                                    ty: "array".to_string(),
                                    items: Box::new(items_schema),
                                    description,
                                    prefix_items: None,
                                    min_items: None,
                                    max_items: None,
                                    example: example.and_then(|v| serde_json::to_string(v).ok()),
                                    default: default_value,
                                    deprecated: None,
                                    enum_values: None,
                                };
                            }
                        }
                    }
                    JsonSchema::Array {
                        ty: "array".to_string(),
                        items: Box::new(JsonSchema::Any {
                            description: None,
                            default: None,
                            deprecated: None,
                        }),
                        description,
                        prefix_items: None,
                        min_items: None,
                        max_items: None,
                        example: example.map(|s| s.to_string()),
                        default: default_value,
                        deprecated: None,
                        enum_values: None,
                    }
                }

                // HashMap<K, V> - 带 additionalProperties 的对象
                "HashMap" => {
                    if let Some(last_segment) = path.path.segments.last() {
                        if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                            if args.args.len() >= 2 {
                                // 获取 key 类型
                                let key_arg = args.args.first().unwrap();
                                // 检查 key 类型是否为 String
                                if let syn::GenericArgument::Type(key_ty) = key_arg {
                                    if !is_string_type(key_ty) {
                                        // 非 String key 的 HashMap 无法正确表示为 JSON object
                                        return JsonSchema::Any {
                                            description: Some(
                                                "HashMap 的 key 类型必须是 String".to_string(),
                                            ),
                                            default: default_value,
                                            deprecated: None,
                                        };
                                    }
                                }

                                // 获取 value 类型
                                if let Some(syn::GenericArgument::Type(value_ty)) =
                                    args.args.iter().nth(1)
                                {
                                    let additional_schema =
                                        generate_schema_for_type_with_default_and_example(
                                            value_ty, None, None, None,
                                        );
                                    return JsonSchema::Object {
                                        ty: "object".to_string(),
                                        properties: BTreeMap::new(),
                                        required: vec![],
                                        description,
                                        additional_properties: Some(Box::new(additional_schema)),
                                        default: default_value,
                                        deprecated: None,
                                        tags: Vec::new(),
                                        returns: None,
                                        replaced_by: None,
                                        context: None,
                                        deprecated_note: None,
                                    };
                                }
                            }
                        }
                    }
                    JsonSchema::Object {
                        ty: "object".to_string(),
                        properties: BTreeMap::new(),
                        required: vec![],
                        description,
                        additional_properties: None,
                        default: default_value,
                        deprecated: None,
                        tags: Vec::new(),
                        returns: None,
                        replaced_by: None,
                        context: None,
                        deprecated_note: None,
                    }
                }

                // 第三方类型特判
                "DateTime" | "NaiveDateTime" | "NaiveDate" | "NaiveTime" => {
                    JsonSchema::string_with_example_and_default(
                        description,
                        Some("date-time".to_string()),
                        example.and_then(|v| serde_json::to_string(v).ok()),
                        default_value,
                    )
                }
                "Uuid" => JsonSchema::string_with_example_and_default(
                    description,
                    Some("uuid".to_string()),
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                ),
                "Url" => JsonSchema::string_with_example_and_default(
                    description,
                    Some("uri".to_string()),
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                ),
                "PathBuf" | "Path" => JsonSchema::string_with_example_and_default(
                    description,
                    Some("file-path".to_string()),
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                ),
                "Value" => JsonSchema::Any {
                    description,
                    default: default_value,
                    deprecated: None,
                },

                // 自定义类型 - 尝试从缓存中获取 schema
                _ => {
                    // 首先检查缓存
                    if let Ok(cache) = TYPE_SCHEMA_CACHE.lock() {
                        if let Some(cached_schema) = cache.get(&ident) {
                            return cached_schema.clone();
                        }
                    }

                    // 缓存未命中，生成基本的 object schema
                    JsonSchema::Object {
                        ty: "object".to_string(),
                        properties: BTreeMap::new(),
                        required: vec![],
                        description: description.or_else(|| Some(format!("自定义类型：{}", ident))),
                        additional_properties: None,
                        default: default_value,
                        deprecated: None,
                        tags: Vec::new(),
                        returns: None,
                        replaced_by: None,
                        context: None,
                        deprecated_note: None,
                    }
                }
            }
        }

        // 引用类型
        Type::Reference(reference) => {
            if let Type::Path(path) = &*reference.elem {
                if let Some(ident) = path.path.segments.first() {
                    if ident.ident == "str" {
                        return JsonSchema::string_with_example_and_default(
                            description,
                            Some("string".to_string()),
                            example.and_then(|v| serde_json::to_string(v).ok()),
                            default_value,
                        );
                    }
                }
            }
            generate_schema_for_type_with_default_and_example(
                &reference.elem,
                description,
                example,
                default,
            )
        }

        // 切片类型
        Type::Slice(slice) => {
            let elem_schema =
                generate_schema_for_type_with_default_and_example(&slice.elem, None, None, None);
            JsonSchema::Array {
                ty: "array".to_string(),
                items: Box::new(elem_schema),
                description,
                prefix_items: None,
                min_items: None,
                max_items: None,
                example: example.and_then(|v| serde_json::to_string(v).ok()),
                default: default_value,
                deprecated: None,
                enum_values: None,
            }
        }

        // 数组类型 [T; N]
        Type::Array(array) => {
            let elem_schema =
                generate_schema_for_type_with_default_and_example(&array.elem, None, None, None);
            let len = match &array.len {
                Expr::Lit(lit) => {
                    if let ExprLit {
                        lit: Lit::Int(int), ..
                    } = lit
                    {
                        int.base10_parse::<usize>().ok()
                    } else {
                        None
                    }
                }
                _ => None,
            };
            JsonSchema::Array {
                ty: "array".to_string(),
                items: Box::new(elem_schema),
                description,
                prefix_items: None,
                min_items: len,
                max_items: len,
                example: example.and_then(|v| serde_json::to_string(v).ok()),
                default: default_value,
                deprecated: None,
                enum_values: None,
            }
        }

        // 元组类型 (T, U, ...)
        Type::Tuple(tuple) => {
            let prefix_items: Vec<JsonSchema> = tuple
                .elems
                .iter()
                .map(|elem| {
                    generate_schema_for_type_with_default_and_example(elem, None, None, None)
                })
                .collect();
            let len = prefix_items.len();
            // 从 prefix_items 中提取统一的 items 类型
            let items_schema = if prefix_items.is_empty() {
                JsonSchema::Any {
                    description: None,
                    default: None,
                    deprecated: None,
                }
            } else {
                // 使用第一个元素的类型作为 items 类型（简化处理）
                prefix_items[0].clone()
            };
            JsonSchema::Array {
                ty: "array".to_string(),
                items: Box::new(items_schema),
                description,
                prefix_items: Some(prefix_items),
                min_items: Some(len),
                max_items: Some(len),
                example: example.and_then(|v| serde_json::to_string(v).ok()),
                default: default_value,
                deprecated: None,
                enum_values: None,
            }
        }

        // 其他类型
        _ => JsonSchema::Any {
            description: description.or_else(|| Some("未知类型".to_string())),
            default: default_value,
            deprecated: None,
        },
    }
}

/// 检查类型是否为 String
fn is_string_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => {
            if let Some(ident) = path.path.segments.first() {
                return ident.ident == "String" || ident.ident == "str";
            }
        }
        Type::Reference(reference) => {
            if let Type::Path(path) = &*reference.elem {
                if let Some(ident) = path.path.segments.first() {
                    return ident.ident == "str";
                }
            }
        }
        _ => {}
    }
    false
}

/// 从函数签名提取参数
fn extract_params(
    inputs: &Punctuated<FnArg, token::Comma>,
    fn_attrs: &[syn::Attribute],
    hidden_params: &[String],
    param_validations: &[(String, ParamToolAttrs)], // 新增：方法级别的参数验证属性
) -> Vec<ParamInfo> {
    let mut params = Vec::new();

    // 从 doc comment 中提取 @param 描述
    let param_docs = extract_param_docs(fn_attrs);

    for arg in inputs {
        if let FnArg::Typed(PatType { pat, ty, attrs, .. }) = arg {
            if let Pat::Ident(ident) = pat.as_ref() {
                if ident.ident == "self" || ident.ident == "_self" {
                    continue;
                }

                let param_name = ident.ident.to_string();

                // 如果参数在 hidden_params 列表中，跳过
                if hidden_params.contains(&param_name) {
                    continue;
                }

                // 去掉 `_` 前缀（Rust 中 unused 参数的命名约定）
                let schema_name = param_name
                    .strip_prefix('_')
                    .unwrap_or(&param_name)
                    .to_string();

                // 优先级：#[tool(desc = "...")] > @param_desc > @param doc comment > 普通 doc comment
                let mut param_tool_attrs = parse_param_tool_attrs(attrs).unwrap_or_default();

                // 合并方法级别的参数验证属性（如 one_of_role, pattern_email 等）
                if let Some(method_level_attrs) = param_validations
                    .iter()
                    .find(|(name, _)| name == &schema_name)
                {
                    // 合并验证属性：方法级别的属性作为默认值，参数级别的属性优先级更高
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
                    .or_else(|| extract_doc_comment(attrs));

                // 检查是否有 required 属性（支持多种方式）
                // 1. #[tool(required)] 或 #[param_tool(required)]
                // 2. #[tool_required]
                // 3. /// @required param_name
                let is_required_explicit = param_tool_attrs.required
                    || extract_param_attr_from_docs(fn_attrs, &schema_name, "required");

                // 获取 example
                let example = param_tool_attrs.example.clone();

                // 获取 default
                let default = param_tool_attrs.default.clone();

                // 获取 validate 和 transform
                // 优先级：#[tool_validate] / #[param_tool(validate = "...")] > @validate doc > 无
                let validate = param_tool_attrs
                    .validate
                    .clone()
                    .or_else(|| extract_param_validate_from_docs(fn_attrs, &schema_name));
                let transform = param_tool_attrs
                    .transform
                    .clone()
                    .or_else(|| extract_param_transform_from_docs(fn_attrs, &schema_name));
                // 获取 validate_msg
                // 优先级：#[tool_validate_msg] / #[param_tool(validate_msg = "...")] > @validate_msg doc > 方法级 validate_msg_ > 无
                let validate_msg = param_tool_attrs
                    .validate_msg
                    .clone()
                    .or_else(|| extract_validate_msg_from_docs(fn_attrs, &schema_name));

                // 获取 JSON Schema 验证属性
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

                // 如果没有显式标记 required，则根据类型判断是否为 Option
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
/// 支持以下属性：
/// - #[tool_required] - 标记参数为必需（即使 Option 类型）
/// - #[tool_desc = "..."] 或 #[tool_desc("...")] - 自定义参数描述
/// - #[tool_example = "..."] 或 #[tool_example("...")] - 示例值（支持字符串或任意字面量）
/// - #[tool_default = "..."] 或 #[tool_default("...")] - 默认值（支持字符串或任意字面量）
/// - #[tool_validate = "..."] 或 #[tool_validate("...")] - 验证表达式
/// - #[tool_transform = "..."] 或 #[tool_transform("...")] - 转换表达式
/// - #[param_tool(validate = "...", desc = "...")] - 参数级工具属性（合并语法）
/// - #[tool(validate = "...", desc = "...")] - 参数级工具属性（替代语法）
fn parse_param_tool_attrs(attrs: &[syn::Attribute]) -> Option<ParamToolAttrs> {
    let mut result = ParamToolAttrs::default();
    let mut found_any = false;

    for attr in attrs {
        // 支持 #[param_tool(...)] 语法
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

        // 支持 #[tool(...)] 语法（参数级别）
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
            // 支持两种语法：#[tool_desc = "..."] 或 #[tool_desc("...")]
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
            // 支持字符串或任意字面量
            if let Ok(_meta) = attr.parse_args::<Expr>() {
                // 尝试解析为字符串字面量
                if let Ok(meta) = attr.parse_args::<LitStr>() {
                    result.example =
                        serde_json::from_str::<serde_json::Value>(&format!("\"{}\"", meta.value()))
                            .ok();
                    found_any = true;
                }
                // 尝试解析为其他字面量
                else if let Ok(meta) = attr.parse_args::<Lit>() {
                    result.example = parse_literal_to_json(&meta);
                    found_any = true;
                }
            }
        } else if attr.path().is_ident("tool_default") {
            // 支持字符串或任意字面量
            if let Ok(_meta) = attr.parse_args::<Expr>() {
                // 尝试解析为字符串字面量
                if let Ok(meta) = attr.parse_args::<LitStr>() {
                    result.default =
                        serde_json::from_str::<serde_json::Value>(&format!("\"{}\"", meta.value()))
                            .ok();
                    found_any = true;
                }
                // 尝试解析为其他字面量
                else if let Ok(meta) = attr.parse_args::<Lit>() {
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
            // 尝试解析为 JSON（如 "null", "42", "true" 等）
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
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.first() {
            return segment.ident == "Option";
        }
    }
    false
}

/// 检查返回类型是否为 Result
fn is_result_type(output: &ReturnType) -> bool {
    match output {
        ReturnType::Type(_, ty) => {
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

/// 提取 doc comment（保留原始文本格式）
///
/// 功能：
/// - 保留原始文本内容，包括 Markdown 标记：**bold**, *italic*, `code`, [links](url)
/// - 支持多段落合并（空行分隔）
/// - 支持结构化注释过滤：# Parameters, # Returns, # Example
/// - 过滤 @param、@required、@param_desc 等参数标记
/// - 支持代码块识别（``` 标记）
///
/// 注意：此函数仅保留原始文本，不进行 Markdown 解析（如转换为 HTML）。
/// 如需完整的 Markdown 支持，建议使用 pulldown-cmark 等外部库。
fn extract_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
    let mut doc_lines = Vec::new();
    let mut in_code_block = false;

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let line = lit.value();
                        let trimmed = line.trim().trim_start_matches(':').trim();

                        // 跟踪代码块状态（``` 标记）
                        if trimmed.starts_with("```") {
                            in_code_block = !in_code_block;
                            doc_lines.push(trimmed.to_string());
                            continue;
                        }

                        // 跳过 @param 行（这些是参数描述，不是工具描述）
                        if trimmed.starts_with("@param") {
                            continue;
                        }

                        // 跳过 @required 行（这些是参数标记，不是工具描述）
                        if trimmed.starts_with("@required") {
                            continue;
                        }

                        // 跳过 @param_desc 行（这些是参数描述，不是工具描述）
                        if trimmed.starts_with("@param_desc") {
                            continue;
                        }

                        // 跳过 - `name`: description 格式的参数行（除非在代码块内）
                        if !in_code_block
                            && trimmed.starts_with('-')
                            && trimmed.contains('`')
                            && trimmed.contains("`:")
                        {
                            continue;
                        }

                        // 跳过 # Parameters / # Returns / # Example 等结构化标记
                        if trimmed.starts_with('#')
                            && (trimmed.contains("Parameters")
                                || trimmed.contains("Returns")
                                || trimmed.contains("Example"))
                        {
                            continue;
                        }

                        // 保留空行用于段落分隔（如果在代码块内或两段之间）
                        if trimmed.is_empty() {
                            if !doc_lines.is_empty()
                                && doc_lines.last().is_some_and(|s| !s.is_empty())
                            {
                                doc_lines.push(String::new());
                            }
                        } else {
                            doc_lines.push(trimmed.to_string());
                        }
                    }
                }
            }
        }
    }

    // 移除末尾的空行
    while doc_lines.last().is_some_and(|s| s.is_empty()) {
        doc_lines.pop();
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}

/// 从 doc comment 中提取 @param 描述
/// 支持格式：
/// - /// @param param_name 描述内容
/// - /// @required - 标记参数为必需
/// - /// @param_desc 描述内容 - 为前一个参数添加描述
fn extract_param_docs(attrs: &[syn::Attribute]) -> BTreeMap<String, String> {
    let mut param_docs = BTreeMap::new();

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        // 支持：/// @param user_id 用户唯一标识
                        if let Some((param_name, desc)) = parse_param_doc(&content) {
                            param_docs.insert(param_name, desc);
                        }
                    }
                }
            }
        }
    }

    param_docs
}

/// 从 doc comment 中提取参数级别的属性（如 @required param_name）
fn extract_param_attr_from_docs(
    attrs: &[syn::Attribute],
    param_name: &str,
    attr_name: &str,
) -> bool {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        // 检查：/// @required param_name
                        if let Some(rest) = trimmed.strip_prefix(&format!("@{}", attr_name)) {
                            let rest = rest.trim();
                            // 检查是否匹配当前参数名
                            if rest == param_name || rest.is_empty() {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// 从 doc comment 中提取 @param_desc param_name desc 描述
fn extract_param_desc_from_docs(attrs: &[syn::Attribute], param_name: &str) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        // 检查：/// @param_desc param_name 描述内容
                        if let Some(rest) = trimmed.strip_prefix("@param_desc") {
                            let rest = rest.trim();
                            // 检查是否匹配当前参数名
                            if let Some(desc) = rest.strip_prefix(param_name) {
                                return Some(desc.trim().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 从 doc comment 中提取 @validate param_name expression
fn extract_param_validate_from_docs(attrs: &[syn::Attribute], param_name: &str) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        // 检查：/// @validate param_name expression
                        if let Some(rest) = trimmed.strip_prefix("@validate") {
                            let rest = rest.trim();
                            // 检查是否匹配当前参数名
                            if let Some(expr) = rest.strip_prefix(param_name) {
                                let expr = expr.trim();
                                if !expr.is_empty() {
                                    return Some(expr.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 从 doc comment 中提取 @transform param_name expression
fn extract_param_transform_from_docs(attrs: &[syn::Attribute], param_name: &str) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        // 检查：/// @transform param_name expression
                        if let Some(rest) = trimmed.strip_prefix("@transform") {
                            let rest = rest.trim();
                            // 检查是否匹配当前参数名
                            if let Some(expr) = rest.strip_prefix(param_name) {
                                let expr = expr.trim();
                                if !expr.is_empty() {
                                    return Some(expr.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 从 doc comment 中提取 @validate_msg param_name "message"
fn extract_validate_msg_from_docs(attrs: &[syn::Attribute], param_name: &str) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let content = lit.value();
                        let trimmed = content.trim();
                        // 检查：/// @validate_msg param_name "message"
                        if let Some(rest) = trimmed.strip_prefix("@validate_msg") {
                            let rest = rest.trim();
                            // 检查是否匹配当前参数名
                            if let Some(msg_part) = rest.strip_prefix(param_name) {
                                let msg_part = msg_part.trim();
                                // 解析字符串字面量
                                if msg_part.starts_with('"') && msg_part.ends_with('"') {
                                    return Some(msg_part[1..msg_part.len() - 1].to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 解析单行 @param 文档
/// 返回 (参数名，描述)
/// 支持格式：
/// - /// @param name description
/// - /// - `name`: description
fn parse_param_doc(line: &str) -> Option<(String, String)> {
    let line = line.trim().trim_start_matches(':').trim();

    // 格式 1: /// @param name description
    if let Some(rest) = line.strip_prefix("@param") {
        let rest = rest.trim();
        // 分割参数名和描述
        if let Some(space_pos) = rest.find(' ') {
            let param_name = rest[..space_pos].trim().to_string();
            let desc = rest[space_pos + 1..].trim().to_string();
            if !param_name.is_empty() && !desc.is_empty() {
                return Some((param_name, desc));
            }
        } else if !rest.is_empty() {
            // 只有参数名，没有描述
            return Some((rest.to_string(), String::new()));
        }
    }

    // 格式 2: /// - `name`: description
    if let Some(rest) = line.strip_prefix('-') {
        let rest = rest.trim();
        if let Some(stripped) = rest.strip_prefix('`') {
            if let Some(end) = stripped.find('`') {
                let param_name = stripped[..end].to_string();
                let desc = stripped[end + 1..]
                    .trim()
                    .trim_start_matches(':')
                    .trim()
                    .to_string();
                if !param_name.is_empty() && !desc.is_empty() {
                    return Some((param_name, desc));
                }
            }
        }
    }

    None
}

// ============================================================================
// tokitai! 配置宏实现
// ============================================================================

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
                                        // 跳过未知属性
                                        let _ = param_content.parse::<syn::Expr>();
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
                        // 跳过未知属性
                        let _ = method_content.parse::<syn::Expr>();
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

/// 配置宏主函数
pub fn config(item: TokenStream) -> TokenStream {
    let config_input = parse_macro_input!(item as ConfigInput);

    let struct_name = &config_input.struct_name;

    // 生成唯一的静态变量名
    let config_init_name = format_ident!("__CONFIG_INIT_{}", struct_name);

    // 生成配置代码 - 目前实现为编译期提示
    // 后续可以通过修改 TOOL_DEFINITIONS 来实现真正的覆盖
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
        // 配置宏展开 - 用于集中配置工具属性
        // 使用 LazyLock 在首次访问时初始化配置
        // 注意：配置会在首次访问 GLOBAL_CONFIG_REGISTRY 时自动应用
        #[used]
        static #config_init_name: ::std::sync::LazyLock<()> = ::std::sync::LazyLock::new(|| {
            #(#method_configs)*
        });
    };

    TokenStream::from(output)
}
