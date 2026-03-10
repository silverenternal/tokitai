//! JSON Schema 中间表示 AST
//!
//! 使用 derive(Serialize) 直接序列化为 JSON

use serde::Serialize;
use std::collections::BTreeMap;

/// JSON Schema 中间表示 AST
/// 使用 derive(Serialize) 直接序列化为 JSON
#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum JsonSchema {
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
    pub fn string(description: Option<String>, format: Option<String>) -> Self {
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
    pub fn string_with_example(
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
    pub fn string_with_example_and_default(
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
    pub fn integer(description: Option<String>) -> Self {
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
    pub fn integer_with_default(
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
    pub fn number(description: Option<String>) -> Self {
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
    pub fn number_with_default(
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
    pub fn boolean(description: Option<String>) -> Self {
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
    pub fn boolean_with_default(
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
    pub fn nullable(inner: JsonSchema) -> Self {
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

    /// 创建可空 schema with description and default
    pub fn nullable_with_description_and_default(
        inner: JsonSchema,
        description: Option<String>,
        default: Option<serde_json::Value>,
    ) -> Self {
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
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// 获取 description（如果存在）
    pub fn description(&self) -> Option<&String> {
        match self {
            JsonSchema::Basic { description, .. } => description.as_ref(),
            JsonSchema::Array { description, .. } => description.as_ref(),
            JsonSchema::Object { description, .. } => description.as_ref(),
            JsonSchema::Nullable { description, .. } => description.as_ref(),
            _ => None,
        }
    }

    /// 设置 description
    pub fn set_description(&mut self, desc: Option<String>) {
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
pub fn flatten_option_schema(schema: JsonSchema) -> JsonSchema {
    match schema {
        JsonSchema::Nullable {
            any_of,
            deprecated,
            description,
            ..
        } => {
            let inner = any_of
                .into_iter()
                .find(|s| !matches!(s, JsonSchema::Any { .. }))
                .unwrap_or(JsonSchema::Any {
                    description: None,
                    default: None,
                    deprecated: None,
                });
            let mut flat = flatten_option_schema(inner);
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
