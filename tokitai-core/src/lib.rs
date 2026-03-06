//! Tokitai Core - 编译期工具定义
//!
//! 提供零运行时依赖的核心类型定义，所有工具信息在编译期生成。
//!
//! # 核心类型
//!
//! - [`ToolDefinition`] - 工具定义，包含名称、描述和输入 schema
//! - [`ToolParameter`] - 工具参数定义
//! - [`ToolError`] - 工具调用错误类型
//! - [`ToolProvider`] - 工具提供者 trait（由 `#[tool]` 宏自动实现）
//!
//! # 使用示例
//!
//! ```rust
//! use tokitai_core::ToolDefinition;
//!
//! // 创建工具定义
//! let tool = ToolDefinition::new(
//!     "add",
//!     "两个数相加",
//!     "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}"
//! );
//!
//! assert_eq!(tool.name, "add");
//! assert_eq!(tool.description, "两个数相加");
//! ```
//!
//! # 无标准库支持
//!
//! 本 crate 支持 `no_std` 环境（禁用 `serde` 特性时）：
//!
//! ```toml
//! [dependencies]
//! tokitai-core = { version = "0.3", default-features = false }
//! ```

#![cfg_attr(not(feature = "serde"), no_std)]
#![allow(dead_code)]

#[cfg(feature = "serde")]
extern crate serde;

#[cfg(feature = "serde")]
extern crate alloc;

#[cfg(feature = "serde")]
pub use serde_types::*;

/// 工具定义 - 描述一个 AI 可调用的工具
///
/// 此结构体通常由 `#[tool]` 宏自动生成，无需手动创建。
///
/// # 字段
///
/// * `name` - 工具名称，用于 AI 调用时识别
/// * `description` - 工具描述，帮助 AI 理解工具用途
/// * `input_schema` - 输入参数的 JSON Schema，用于验证参数格式
///
/// # 示例
///
/// ```rust
/// use tokitai_core::ToolDefinition;
///
/// let tool = ToolDefinition::new("add", "Add two numbers", "{\"type\":\"object\"}");
/// assert_eq!(tool.name, "add");
/// ```
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
    ///
    /// # 参数
    ///
    /// * `name` - 工具名称
    /// * `description` - 工具描述
    /// * `input_schema` - JSON Schema 字符串
    ///
    /// # 示例
    ///
    /// ```rust
    /// use tokitai_core::ToolDefinition;
    ///
    /// let tool = ToolDefinition::new(
    ///     "get_weather",
    ///     "获取指定城市的天气",
    ///     "{\"type\":\"object\",\"properties\":{\"city\":{\"type\":\"string\"}},\"required\":[\"city\"]}"
    /// );
    /// ```
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

impl std::fmt::Display for ToolDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.description)
    }
}

/// 参数类型
///
/// 用于描述工具参数的 JSON Schema 类型。
///
/// # 示例
///
/// ```rust
/// use tokitai_core::ParamType;
///
/// assert_eq!(ParamType::from_rust_type("String"), Some(ParamType::String));
/// assert_eq!(ParamType::from_rust_type("i32"), Some(ParamType::Integer));
/// assert_eq!(ParamType::Integer.as_str(), "integer");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ParamType {
    /// 字符串类型
    String = 0,
    /// 整数类型
    Integer = 1,
    /// 数字类型（浮点数）
    Number = 2,
    /// 布尔类型
    Boolean = 3,
    /// 数组类型
    Array = 4,
    /// 对象类型
    Object = 5,
}

impl ParamType {
    /// 获取 JSON Schema 类型字符串
    ///
    /// # 示例
    ///
    /// ```rust
    /// use tokitai_core::ParamType;
    ///
    /// assert_eq!(ParamType::String.as_str(), "string");
    /// assert_eq!(ParamType::Integer.as_str(), "integer");
    /// ```
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
    ///
    /// # 参数
    ///
    /// * `type_name` - Rust 类型名称（如 `"String"`, `"i32"`, `"Vec<i32>"`）
    ///
    /// # 示例
    ///
    /// ```rust
    /// use tokitai_core::ParamType;
    ///
    /// assert_eq!(ParamType::from_rust_type("String"), Some(ParamType::String));
    /// assert_eq!(ParamType::from_rust_type("i32"), Some(ParamType::Integer));
    /// assert_eq!(ParamType::from_rust_type("f64"), Some(ParamType::Number));
    /// assert_eq!(ParamType::from_rust_type("bool"), Some(ParamType::Boolean));
    /// assert_eq!(ParamType::from_rust_type("Vec<i32>"), Some(ParamType::Array));
    /// ```
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
///
/// 用于描述工具的单个参数。
///
/// # 示例
///
/// ```rust
/// use tokitai_core::{ToolParameter, ParamType};
///
/// let param = ToolParameter::new(
///     "city",
///     ParamType::String,
///     "城市名称",
///     true, // 必需参数
/// );
/// ```
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
    ///
    /// # 参数
    ///
    /// * `name` - 参数名称
    /// * `param_type` - 参数类型
    /// * `description` - 参数描述
    /// * `required` - 是否必需
    ///
    /// # 示例
    ///
    /// ```rust
    /// use tokitai_core::{ToolParameter, ParamType};
    ///
    /// let param = ToolParameter::new("limit", ParamType::Integer, "返回结果数量", false);
    /// ```
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
///
/// 表示工具调用过程中可能发生的错误。
///
/// # 示例
///
/// ```rust
/// use tokitai_core::{ToolError, ToolErrorKind};
///
/// let error = ToolError::validation_error("缺少必需参数 'city'");
/// assert_eq!(error.kind, ToolErrorKind::ValidationError);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolError {
    /// 错误类型
    pub kind: ToolErrorKind,
    /// 错误消息
    #[cfg(feature = "serde")]
    pub message: crate::serde_types::String,
    #[cfg(not(feature = "serde"))]
    pub message: &'static str,
}

#[cfg(feature = "serde")]
impl std::error::Error for ToolError {}

#[cfg(feature = "serde")]
impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ToolError: {:?} - {}", self.kind, self.message)
    }
}

#[cfg(not(feature = "serde"))]
impl ToolError {
    /// 创建新的错误
    pub fn new(kind: ToolErrorKind, message: &'static str) -> Self {
        Self { kind, message }
    }

    /// 创建验证错误
    pub fn validation_error(message: &'static str) -> Self {
        Self {
            kind: ToolErrorKind::ValidationError,
            message,
        }
    }

    /// 创建未找到错误
    pub fn not_found(message: &'static str) -> Self {
        Self {
            kind: ToolErrorKind::NotFound,
            message,
        }
    }

    /// 创建内部错误
    pub fn internal_error(message: &'static str) -> Self {
        Self {
            kind: ToolErrorKind::InternalError,
            message,
        }
    }
}

#[cfg(feature = "serde")]
impl ToolError {
    /// 创建新的错误
    pub fn new(kind: ToolErrorKind, message: impl Into<crate::serde_types::String>) -> Self {
        Self { kind, message: message.into() }
    }

    /// 创建验证错误
    pub fn validation_error(message: impl Into<crate::serde_types::String>) -> Self {
        Self {
            kind: ToolErrorKind::ValidationError,
            message: message.into(),
        }
    }

    /// 创建未找到错误
    pub fn not_found(message: impl Into<crate::serde_types::String>) -> Self {
        Self {
            kind: ToolErrorKind::NotFound,
            message: message.into(),
        }
    }

    /// 创建内部错误
    pub fn internal_error(message: impl Into<crate::serde_types::String>) -> Self {
        Self {
            kind: ToolErrorKind::InternalError,
            message: message.into(),
        }
    }
}

/// 错误类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ToolErrorKind {
    /// 验证错误 - 参数验证失败
    ValidationError = 0,
    /// 工具未找到 - 请求的工具不存在
    NotFound = 1,
    /// 内部错误 - 工具执行失败
    InternalError = 2,
    /// 类型错误 - 参数类型不匹配
    TypeError = 3,
}

/// 编译期工具注册表 trait
///
/// 由 `#[tool]` 宏自动实现，用于提供工具定义和调用接口。
///
/// # 示例
///
/// ```rust
/// use tokitai_core::ToolProvider;
///
/// // 假设有一个使用了 #[tool] 宏的类型
/// // struct Calculator;
/// // #[tool] impl Calculator { ... }
///
/// // 获取所有工具定义
/// // let tools = Calculator::TOOL_DEFINITIONS;
///
/// // 获取工具数量
/// // let count = Calculator::tool_count();
///
/// // 查找特定工具
/// // let tool = Calculator::find_tool("add");
/// ```
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
    //! serde 相关类型别名
    //!
    //! 此模块在使用 `serde` 特性时可用。

    pub use serde_json::Value;
    pub use alloc::string::String;
}

/// 生成 JSON Schema 的辅助宏（编译期）
///
/// 此宏用于在编译期生成 JSON Schema 字符串，避免运行时开销。
///
/// # 示例
///
/// ```rust,ignore
/// // 注意：此宏需要在编译期生成字符串，语法较为特殊
/// use tokitai_core::json_schema;
///
/// const SCHEMA: &str = json_schema!({
///     "city": {
///         type: String,
///         description: "城市名称",
///         required: true,
///     }
/// });
/// ```
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
