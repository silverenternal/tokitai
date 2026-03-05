//! Tokitai Core - 编译期工具定义
//!
//! 提供零运行时依赖的核心类型定义，所有工具信息在编译期生成。
//!
//! # 核心类型
//!
//! - [`ToolDefinition`] - 工具定义，包含名称、描述和输入 schema
//! - [`ToolParameter`] - 工具参数定义
//! - [`ToolError`] - 工具调用错误类型

#![cfg_attr(not(feature = "serde"), no_std)]
#![allow(dead_code)]

#[cfg(feature = "serde")]
extern crate serde;

#[cfg(feature = "serde")]
pub use serde_types::*;

/// 工具定义 - 描述一个 AI 可调用的工具
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolDefinition {
    /// 工具名称
    pub name: &'static str,
    /// 工具描述
    pub description: &'static str,
    /// 输入参数 schema（编译期生成的 JSON 字符串）
    pub input_schema: &'static str,
}

impl ToolDefinition {
    /// 创建新的工具定义
    pub fn new(
        name: &'static str,
        description: &'static str,
        input_schema: &'static str,
    ) -> Self {
        Self {
            name,
            description,
            input_schema,
        }
    }

    /// 转换为 JSON 字符串（需要 serde 特性）
    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 转换为 JSON Value（需要 serde 特性）
    #[cfg(feature = "serde")]
    pub fn to_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

/// 参数类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ParamType {
    String = 0,
    Integer = 1,
    Number = 2,
    Boolean = 3,
    Array = 4,
    Object = 5,
}

impl ParamType {
    /// 获取 JSON Schema 类型字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            ParamType::String => "string",
            ParamType::Integer => "integer",
            ParamType::Number => "number",
            ParamType::Boolean => "boolean",
            ParamType::Array => "array",
            ParamType::Object => "object",
        }
    }

    /// 从 Rust 类型名推断参数类型
    pub fn from_rust_type(type_name: &str) -> Option<Self> {
        match type_name {
            "String" | "str" => Some(ParamType::String),
            "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "isize" => {
                Some(ParamType::Integer)
            }
            "f32" | "f64" => Some(ParamType::Number),
            "bool" => Some(ParamType::Boolean),
            _ => {
                if type_name.starts_with("Vec<") {
                    Some(ParamType::Array)
                } else if type_name.starts_with("Option<") {
                    None
                } else {
                    Some(ParamType::Object)
                }
            }
        }
    }
}

/// 工具参数定义
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolParameter {
    /// 参数名称
    pub name: &'static str,
    /// 参数类型
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub param_type: ParamType,
    /// 参数描述
    pub description: &'static str,
    /// 是否必需
    pub required: bool,
}

impl ToolParameter {
    /// 创建新的参数定义
    pub fn new(
        name: &'static str,
        param_type: ParamType,
        description: &'static str,
        required: bool,
    ) -> Self {
        Self {
            name,
            param_type,
            description,
            required,
        }
    }
}

/// 工具调用错误
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolError {
    /// 错误类型
    pub kind: ToolErrorKind,
    /// 错误消息
    pub message: &'static str,
}

impl ToolError {
    pub fn new(kind: ToolErrorKind, message: &'static str) -> Self {
        Self { kind, message }
    }

    pub fn validation_error(message: &'static str) -> Self {
        Self {
            kind: ToolErrorKind::ValidationError,
            message,
        }
    }

    pub fn not_found(message: &'static str) -> Self {
        Self {
            kind: ToolErrorKind::NotFound,
            message,
        }
    }

    pub fn internal_error(message: &'static str) -> Self {
        Self {
            kind: ToolErrorKind::InternalError,
            message,
        }
    }
}

/// 错误类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ToolErrorKind {
    /// 验证错误
    ValidationError = 0,
    /// 工具未找到
    NotFound = 1,
    /// 内部错误
    InternalError = 2,
    /// 类型错误
    TypeError = 3,
}

/// 编译期工具注册表 trait
///
/// 由 `#[tool]` 宏自动实现
pub trait ToolProvider {
    /// 获取所有工具定义
    fn tool_definitions() -> &'static [ToolDefinition];

    /// 获取工具数量
    fn tool_count() -> usize {
        Self::tool_definitions().len()
    }

    /// 根据名称查找工具定义
    fn find_tool(name: &str) -> Option<&'static ToolDefinition> {
        Self::tool_definitions()
            .iter()
            .find(|t| t.name == name)
    }
}

#[cfg(feature = "serde")]
pub mod serde_types {
    pub use serde_json::Value;
}

/// 生成 JSON Schema 的辅助宏（编译期）
#[macro_export]
macro_rules! json_schema {
    (
        {
            $($param_name:literal: {
                type: $param_type:ident,
                description: $description:literal,
                required: $required:literal $(,)?
            }),*
            $(,)?
        }
    ) => {{
        const SCHEMA: &str = concat!(
            "{\"type\":\"object\",\"properties\":{",
            $({
                concat!(
                    "\"", $param_name, "\":",
                    "{\"type\":\"", $crate::ParamType::$param_type.as_str(), "\",\"description\":\"", $description, "\"}"
                )
            },)*
            "},\"required\":[",
            $({
                if $required { concat!("\"", $param_name, "\"") } else { "" }
            },)*
            "]}"
        );
        SCHEMA
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_type_from_rust_type() {
        assert_eq!(ParamType::from_rust_type("String"), Some(ParamType::String));
        assert_eq!(ParamType::from_rust_type("i32"), Some(ParamType::Integer));
        assert_eq!(ParamType::from_rust_type("f64"), Some(ParamType::Number));
        assert_eq!(ParamType::from_rust_type("bool"), Some(ParamType::Boolean));
        assert_eq!(ParamType::from_rust_type("Vec<i32>"), Some(ParamType::Array));
    }

    #[test]
    fn test_tool_definition_const() {
        let tool = ToolDefinition {
            name: "test",
            description: "A test tool",
            input_schema: "{}",
        };
        assert_eq!(tool.name, "test");
        assert_eq!(tool.description, "A test tool");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_tool_definition_to_json() {
        let tool = ToolDefinition::new("test", "A test tool", "{\"type\":\"object\"}");
        let json = tool.to_json().unwrap();
        assert!(json.contains("\"name\":\"test\""));
    }
}
