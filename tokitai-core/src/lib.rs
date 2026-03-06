//! # Tokitai Core
//!
//! **Core types for Tokitai - Compile-time tool definitions with zero runtime dependencies**
//!
//! This crate provides the fundamental types and traits for the Tokitai AI tool integration system.
//! All tool information is generated at compile time, ensuring zero runtime overhead and maximum
//! type safety.
//!
//! ## 🎯 Key Features
//!
//! - **Zero Runtime Dependencies** - Core types have minimal dependencies
//! - **`no_std` Support** - Works in embedded environments (when `serde` feature is disabled)
//! - **Type Safety** - Compile-time tool definitions prevent runtime errors
//! - **Serde Integration** - Optional serialization support via the `serde` feature
//!
//! ## Core Types
//!
//! - [`ToolDefinition`] - Tool definition containing name, description, and input schema
//! - [`ToolParameter`] - Parameter definition for tools
//! - [`ParamType`] - JSON Schema type enumeration
//! - [`ToolError`] - Error type for tool invocation failures
//! - [`ToolErrorKind`] - Classification of tool errors
//! - [`ToolProvider`] - Trait for tool providers (auto-implemented by `#[tool]` macro)
//!
//! ## Usage Example
//!
//! ```rust
//! use tokitai_core::ToolDefinition;
//!
//! // Create a tool definition
//! let tool = ToolDefinition::new(
//!     "add",
//!     "Add two numbers together",
//!     r#"{"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}},"required":["a","b"]}"#
//! );
//!
//! assert_eq!(tool.name, "add");
//! assert_eq!(tool.description, "Add two numbers together");
//!
//! // With serde feature enabled, convert to JSON
//! #[cfg(feature = "serde")]
//! {
//!     let json = tool.to_json().unwrap();
//!     assert!(json.contains("\"name\":\"add\""));
//! }
//! ```
//!
//! ## No-Std Support
//!
//! This crate supports `no_std` environments when the `serde` feature is disabled:
//!
//! ```toml
//! [dependencies]
//! tokitai-core = { version = "0.3", default-features = false }
//! ```
//!
//! ## Type Mapping
//!
//! The [`ParamType`] enum maps Rust types to JSON Schema types:
//!
//! | Rust Type | JSON Schema Type | `ParamType` Variant |
//! |-----------|------------------|---------------------|
//! | `String`, `&str` | `string` | `ParamType::String` |
//! | `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | `integer` | `ParamType::Integer` |
//! | `f32`, `f64` | `number` | `ParamType::Number` |
//! | `bool` | `boolean` | `ParamType::Boolean` |
//! | `Vec<T>` | `array` | `ParamType::Array` |
//! | Custom structs | `object` | `ParamType::Object` |
//!
//! ```rust
//! use tokitai_core::ParamType;
//!
//! assert_eq!(ParamType::from_rust_type("String"), Some(ParamType::String));
//! assert_eq!(ParamType::from_rust_type("i32"), Some(ParamType::Integer));
//! assert_eq!(ParamType::from_rust_type("f64"), Some(ParamType::Number));
//! assert_eq!(ParamType::from_rust_type("bool"), Some(ParamType::Boolean));
//! assert_eq!(ParamType::from_rust_type("Vec<i32>"), Some(ParamType::Array));
//! ```
//!
//! ## Error Handling
//!
//! The [`ToolError`] type provides structured error handling for tool invocations:
//!
//! ```rust
//! use tokitai_core::{ToolError, ToolErrorKind};
//!
//! // Create different error types
//! let validation_err = ToolError::validation_error("Missing required parameter 'city'");
//! assert_eq!(validation_err.kind, ToolErrorKind::ValidationError);
//!
//! let not_found_err = ToolError::not_found("Tool 'unknown_tool' not found");
//! assert_eq!(not_found_err.kind, ToolErrorKind::NotFound);
//!
//! let internal_err = ToolError::internal_error("Connection timeout");
//! assert_eq!(internal_err.kind, ToolErrorKind::InternalError);
//! ```
//!
//! ## Tool Provider Trait
//!
//! The [`ToolProvider`] trait is automatically implemented by the `#[tool]` macro:
//!
//! ```rust
//! use tokitai_core::ToolProvider;
//!
//! // After using #[tool] macro on your type:
//! // struct Calculator;
//! // #[tool] impl Calculator { ... }
//!
//! // Get all tool definitions
//! // let tools = Calculator::tool_definitions();
//!
//! // Get tool count
//! // let count = Calculator::tool_count();
//!
//! // Find a specific tool
//! // let tool = Calculator::find_tool("add");
//! ```
//!
//! ## JSON Schema Macro
//!
//! The `json_schema!` macro helps generate JSON Schema strings at compile time:
//!
//! ```rust,ignore
//! use tokitai_core::json_schema;
//!
//! const SCHEMA: &str = json_schema!({
//!     "city": {
//!         type: String,
//!         description: "Name of the city",
//!         required: true,
//!     }
//! });
//! ```
//!
//! ## Features
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `serde` (default) | Enable serde serialization and JSON support |
//!
//! ## Requirements
//!
//! - **Rust Version**: 1.70+
//! - **Edition**: 2021
//!
//! ## License
//!
//! Licensed under either of:
//!
//! - Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/silverenternal/tokitai/blob/main/LICENSE))
//! - MIT License ([LICENSE-MIT](https://github.com/silverenternal/tokitai/blob/main/LICENSE))
//!
//! at your option.
//!
//! ## Contributing
//!
//! Unless you explicitly state otherwise, any contribution intentionally submitted
//! for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be
//! dual licensed as above, without any additional terms or conditions.
//!
//! ## See Also
//!
//! - [`tokitai`](https://crates.io/crates/tokitai) - Main crate with runtime support
//! - [`tokitai-macros`](https://crates.io/crates/tokitai-macros) - Procedural macros

#![cfg_attr(not(feature = "serde"), no_std)]
#![allow(dead_code)]

#[cfg(feature = "serde")]
extern crate serde;

#[cfg(feature = "serde")]
extern crate alloc;

#[cfg(feature = "serde")]
pub use serde_types::*;

/// # Tool Definition
///
/// Represents a tool that can be called by an AI system.
///
/// This struct is typically generated automatically by the `#[tool]` macro,
/// so manual creation is rarely needed.
///
/// ## Fields
///
/// - `name` - The tool identifier used for AI recognition
/// - `description` - Human-readable description helping AI understand the tool's purpose
/// - `input_schema` - JSON Schema string for parameter validation
///
/// ## Example
///
/// ```rust
/// use tokitai_core::ToolDefinition;
///
/// let tool = ToolDefinition::new("add", "Add two numbers", r#"{"type":"object"}"#);
/// assert_eq!(tool.name, "add");
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolDefinition {
    /// Tool name used for identification during AI calls
    pub name: &'static str,
    /// Tool description helping AI understand its purpose
    pub description: &'static str,
    /// Input parameter JSON Schema (compile-time generated string)
    pub input_schema: &'static str,
}

impl ToolDefinition {
    /// Create a new tool definition
    ///
    /// # Parameters
    ///
    /// - `name` - Tool name
    /// - `description` - Tool description
    /// - `input_schema` - JSON Schema string
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolDefinition;
    ///
    /// let tool = ToolDefinition::new(
    ///     "get_weather",
    ///     "Get weather information for a specified city",
    ///     r#"{"type":"object","properties":{"city":{"type":"string"}},"required":["city"]}"#
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

    /// Convert to JSON string (requires `serde` feature)
    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Convert to JSON Value (requires `serde` feature)
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

/// # Parameter Type
///
/// Represents JSON Schema types for tool parameters.
///
/// ## Example
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
    /// String type
    String = 0,
    /// Integer type
    Integer = 1,
    /// Number type (floating point)
    Number = 2,
    /// Boolean type
    Boolean = 3,
    /// Array type
    Array = 4,
    /// Object type
    Object = 5,
}

impl ParamType {
    /// Get the JSON Schema type string
    ///
    /// # Example
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

    /// Infer parameter type from Rust type name
    ///
    /// # Parameters
    ///
    /// - `type_name` - Rust type name (e.g., `"String"`, `"i32"`, `"Vec<i32>"`)
    ///
    /// # Example
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

/// # Tool Parameter
///
/// Represents a single parameter definition for a tool.
///
/// ## Example
///
/// ```rust
/// use tokitai_core::{ToolParameter, ParamType};
///
/// let param = ToolParameter::new(
///     "city",
///     ParamType::String,
///     "Name of the city",
///     true, // Required parameter
/// );
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolParameter {
    /// Parameter name
    pub name: &'static str,
    /// Parameter type
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub param_type: ParamType,
    /// Parameter description
    pub description: &'static str,
    /// Whether the parameter is required
    pub required: bool,
}

impl ToolParameter {
    /// Create a new parameter definition
    ///
    /// # Parameters
    ///
    /// - `name` - Parameter name
    /// - `param_type` - Parameter type
    /// - `description` - Parameter description
    /// - `required` - Whether the parameter is required
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::{ToolParameter, ParamType};
    ///
    /// let param = ToolParameter::new("limit", ParamType::Integer, "Number of results to return", false);
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

/// # Tool Error
///
/// Represents errors that can occur during tool invocation.
///
/// ## Example
///
/// ```rust
/// use tokitai_core::{ToolError, ToolErrorKind};
///
/// let error = ToolError::validation_error("Missing required parameter 'city'");
/// assert_eq!(error.kind, ToolErrorKind::ValidationError);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolError {
    /// Error type classification
    pub kind: ToolErrorKind,
    /// Error message
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
    /// Create a new error
    pub fn new(kind: ToolErrorKind, message: &'static str) -> Self {
        Self { kind, message }
    }

    /// Create a validation error
    pub fn validation_error(message: &'static str) -> Self {
        Self {
            kind: ToolErrorKind::ValidationError,
            message,
        }
    }

    /// Create a not found error
    pub fn not_found(message: &'static str) -> Self {
        Self {
            kind: ToolErrorKind::NotFound,
            message,
        }
    }

    /// Create an internal error
    pub fn internal_error(message: &'static str) -> Self {
        Self {
            kind: ToolErrorKind::InternalError,
            message,
        }
    }
}

#[cfg(feature = "serde")]
impl ToolError {
    /// Create a new error
    pub fn new(kind: ToolErrorKind, message: impl Into<crate::serde_types::String>) -> Self {
        Self { kind, message: message.into() }
    }

    /// Create a validation error
    pub fn validation_error(message: impl Into<crate::serde_types::String>) -> Self {
        Self {
            kind: ToolErrorKind::ValidationError,
            message: message.into(),
        }
    }

    /// Create a not found error
    pub fn not_found(message: impl Into<crate::serde_types::String>) -> Self {
        Self {
            kind: ToolErrorKind::NotFound,
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal_error(message: impl Into<crate::serde_types::String>) -> Self {
        Self {
            kind: ToolErrorKind::InternalError,
            message: message.into(),
        }
    }
}

/// # Error Kind
///
/// Classification of tool errors for structured error handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ToolErrorKind {
    /// Validation error - parameter validation failed
    ValidationError = 0,
    /// Not found - requested tool does not exist
    NotFound = 1,
    /// Internal error - tool execution failed
    InternalError = 2,
    /// Type error - parameter type mismatch
    TypeError = 3,
}

/// # Compile-time Tool Registry Trait
///
/// Automatically implemented by the `#[tool]` macro, providing tool definitions
/// and invocation interface.
///
/// ## Example
///
/// ```rust
/// use tokitai_core::ToolProvider;
///
/// // After using #[tool] macro on your type:
/// // struct Calculator;
/// // #[tool] impl Calculator { ... }
///
/// // Get all tool definitions
/// // let tools = Calculator::tool_definitions();
///
/// // Get tool count
/// // let count = Calculator::tool_count();
///
/// // Find a specific tool
/// // let tool = Calculator::find_tool("add");
/// ```
pub trait ToolProvider {
    /// Get all tool definitions
    fn tool_definitions() -> &'static [ToolDefinition];

    /// Get the number of tools
    fn tool_count() -> usize {
        Self::tool_definitions().len()
    }

    /// Find a tool definition by name
    fn find_tool(name: &str) -> Option<&'static ToolDefinition> {
        Self::tool_definitions()
            .iter()
            .find(|t| t.name == name)
    }
}

#[cfg(feature = "serde")]
pub mod serde_types {
    //! Serde type aliases
    //!
    //! This module is available when the `serde` feature is enabled.

    pub use serde_json::Value;
    pub use alloc::string::String;
}

/// # JSON Schema Macro (Compile-time)
///
/// Helper macro for generating JSON Schema strings at compile time,
/// avoiding runtime overhead.
///
/// ## Example
///
/// ```rust,ignore
/// // Note: This macro generates strings at compile time, syntax is special
/// use tokitai_core::json_schema;
///
/// const SCHEMA: &str = json_schema!({
///     "city": {
///         type: String,
///         description: "Name of the city",
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
        let tool = ToolDefinition::new("test", "A test tool", r#"{"type":"object"}"#);
        let json = tool.to_json().unwrap();
        assert!(json.contains(r#""name":"test""#));
    }
}
