//! Schema 生成逻辑
//!
//! 包含 SchemaGenConfig、generate_schema_json_with_deprecated_and_tags、
//! generate_schema_for_type 系列函数

use std::collections::BTreeMap;

use super::types::JsonSchema;
use crate::tool::types::param::ParamInfo;

/// JSON Schema 生成配置（Builder 模式）
pub struct SchemaGenConfig<'a> {
    pub params: &'a [ParamInfo],
    pub deprecated: bool,
    pub replaced_by: Option<&'a str>,
    pub context: Option<&'a str>,
    pub tags: &'a [String],
    pub return_description: Option<&'a str>,
    pub example_input: Option<&'a serde_json::Value>,
    pub param_order: Option<&'a [String]>,
    pub example_output: Option<&'a str>,
    pub deprecated_note: Option<&'a str>,
    pub deprecated_since: Option<&'a str>,
    pub remove_in: Option<&'a str>,
    pub group: Option<&'a str>,
    pub cache: Option<&'a str>,
    pub rate_limit: Option<&'a str>,
}

impl<'a> SchemaGenConfig<'a> {
    pub fn new(params: &'a [ParamInfo]) -> Self {
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

    pub(crate) fn deprecated(mut self, val: bool) -> Self {
        self.deprecated = val;
        self
    }

    pub(crate) fn replaced_by(mut self, val: Option<&'a str>) -> Self {
        self.replaced_by = val;
        self
    }

    pub(crate) fn context(mut self, val: Option<&'a str>) -> Self {
        self.context = val;
        self
    }

    pub(crate) fn tags(mut self, val: &'a [String]) -> Self {
        self.tags = val;
        self
    }

    pub(crate) fn return_description(mut self, val: Option<&'a str>) -> Self {
        self.return_description = val;
        self
    }

    pub(crate) fn example_input(mut self, val: Option<&'a serde_json::Value>) -> Self {
        self.example_input = val;
        self
    }

    pub(crate) fn param_order(mut self, val: Option<&'a [String]>) -> Self {
        self.param_order = val;
        self
    }

    pub(crate) fn example_output(mut self, val: Option<&'a str>) -> Self {
        self.example_output = val;
        self
    }

    pub(crate) fn deprecated_note(mut self, val: Option<&'a str>) -> Self {
        self.deprecated_note = val;
        self
    }

    pub(crate) fn deprecated_since(mut self, val: Option<&'a str>) -> Self {
        self.deprecated_since = val;
        self
    }

    pub(crate) fn remove_in(mut self, val: Option<&'a str>) -> Self {
        self.remove_in = val;
        self
    }

    pub(crate) fn group(mut self, val: Option<&'a str>) -> Self {
        self.group = val;
        self
    }

    pub(crate) fn cache(mut self, val: Option<&'a str>) -> Self {
        self.cache = val;
        self
    }

    pub(crate) fn rate_limit(mut self, val: Option<&'a str>) -> Self {
        self.rate_limit = val;
        self
    }

    /// 构建配置对象（链式调用终点）
    ///
    /// 注意：此方法是可选的，因为所有 Builder 方法都返回 `Self`，
    /// 可以直接使用链式调用的最终结果。此方法主要用于明确链式调用的结束。
    #[allow(dead_code)]
    pub(crate) fn build(self) -> Self {
        self
    }
}

/// 生成 JSON Schema（支持 deprecated、tags、return_description、example_input、param_order、example_output、deprecated_note、deprecated_since、remove_in、group）
pub fn generate_schema_json_with_deprecated_and_tags(config: &SchemaGenConfig) -> String {
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

        if schema.description().is_none() && p.description.is_some() {
            schema.set_description(p.description.clone());
        }

        properties.insert(schema_name.clone(), schema);

        if p.is_required || !p.is_option {
            required.push(schema_name);
        }
    }

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
pub fn generate_schema_json(params: &[ParamInfo]) -> String {
    generate_schema_json_with_deprecated_and_tags(&SchemaGenConfig::new(params))
}

/// 为类型生成 JSON Schema（递归解析）
#[allow(dead_code)]
pub fn generate_schema_for_type(ty: &syn::Type, description: Option<String>) -> JsonSchema {
    generate_schema_for_type_with_default_and_example(ty, description, None, None)
}

/// 为类型生成 JSON Schema（递归解析，支持 example）
#[allow(dead_code)]
pub fn generate_schema_for_type_with_example(
    ty: &syn::Type,
    description: Option<String>,
    example: Option<&serde_json::Value>,
) -> JsonSchema {
    generate_schema_for_type_with_default_and_example(ty, description, example, None)
}

/// 为类型生成 JSON Schema（递归解析，支持 example 和 default）
pub fn generate_schema_for_type_with_default_and_example(
    ty: &syn::Type,
    description: Option<String>,
    example: Option<&serde_json::Value>,
    default: Option<&serde_json::Value>,
) -> JsonSchema {
    let default_value = default.cloned();

    match ty {
        syn::Type::Path(path) => {
            let ident = path
                .path
                .segments
                .first()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();

            match ident.as_str() {
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

                "Option" => {
                    if let Some(last_segment) = path.path.segments.last() {
                        if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                let inner_schema =
                                    generate_schema_for_type_with_default_and_example(
                                        inner_ty,
                                        description.clone(),
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

                "HashMap" => {
                    if let Some(last_segment) = path.path.segments.last() {
                        if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                            if args.args.len() >= 2 {
                                let key_arg = args.args.first().unwrap();
                                if let syn::GenericArgument::Type(key_ty) = key_arg {
                                    if !is_string_type(key_ty) {
                                        return JsonSchema::Any {
                                            description: Some(
                                                "HashMap 的 key 类型必须是 String".to_string(),
                                            ),
                                            default: default_value,
                                            deprecated: None,
                                        };
                                    }
                                }

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

                _ => {
                    if let Ok(cache) = super::cache::TYPE_SCHEMA_CACHE.lock() {
                        if let Some(cached_schema) = cache.get(&ident) {
                            return cached_schema.clone();
                        }
                    }

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

        syn::Type::Reference(reference) => {
            if let syn::Type::Path(path) = &*reference.elem {
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

        syn::Type::Slice(slice) => {
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

        syn::Type::Array(array) => {
            let elem_schema =
                generate_schema_for_type_with_default_and_example(&array.elem, None, None, None);
            let len = match &array.len {
                syn::Expr::Lit(lit) => {
                    if let syn::ExprLit {
                        lit: syn::Lit::Int(int),
                        ..
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

        syn::Type::Tuple(tuple) => {
            let prefix_items: Vec<JsonSchema> = tuple
                .elems
                .iter()
                .map(|elem| {
                    generate_schema_for_type_with_default_and_example(elem, None, None, None)
                })
                .collect();
            let len = prefix_items.len();
            let items_schema = if prefix_items.is_empty() {
                JsonSchema::Any {
                    description: None,
                    default: None,
                    deprecated: None,
                }
            } else {
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

        _ => JsonSchema::Any {
            description: description.or_else(|| Some("未知类型".to_string())),
            default: default_value,
            deprecated: None,
        },
    }
}

/// 检查类型是否为 String
pub fn is_string_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => {
            if let Some(ident) = path.path.segments.first() {
                return ident.ident == "String" || ident.ident == "str";
            }
        }
        syn::Type::Reference(reference) => {
            if let syn::Type::Path(path) = &*reference.elem {
                if let Some(ident) = path.path.segments.first() {
                    return ident.ident == "str";
                }
            }
        }
        _ => {}
    }
    false
}
