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
//! tokitai-core = { version = "0.4.0", default-features = false }
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
//! | `async` | Enable async runtime support with thread pool |
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

#[cfg(feature = "serde")]
pub use config::{ToolConfig, ToolConfigRegistry, GLOBAL_CONFIG_REGISTRY};

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
    #[cfg(feature = "serde")]
    pub name: alloc::string::String,
    #[cfg(not(feature = "serde"))]
    pub name: &'static str,
    /// Tool description helping AI understand its purpose
    #[cfg(feature = "serde")]
    pub description: alloc::string::String,
    #[cfg(not(feature = "serde"))]
    pub description: &'static str,
    /// Input parameter JSON Schema (compile-time generated string)
    #[cfg(feature = "serde")]
    pub input_schema: alloc::string::String,
    #[cfg(not(feature = "serde"))]
    pub input_schema: &'static str,
    /// Tool version (optional)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg(feature = "serde")]
    pub version: Option<alloc::string::String>,
    #[cfg(not(feature = "serde"))]
    pub version: Option<&'static str>,
    /// Version since when the tool is deprecated (optional)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg(feature = "serde")]
    pub deprecated_since: Option<alloc::string::String>,
    #[cfg(not(feature = "serde"))]
    pub deprecated_since: Option<&'static str>,
    /// Version when the tool will be removed (optional)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg(feature = "serde")]
    pub remove_in: Option<alloc::string::String>,
    #[cfg(not(feature = "serde"))]
    pub remove_in: Option<&'static str>,
    /// Tool that replaces this deprecated tool (optional)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg(feature = "serde")]
    pub replaced_by: Option<alloc::string::String>,
    #[cfg(not(feature = "serde"))]
    pub replaced_by: Option<&'static str>,
}

/// Internal struct for compile-time tool definition data
///
/// This is used to store tool definitions as `&'static str` at compile time,
/// then convert to `ToolDefinition` at runtime with zero allocation.
#[doc(hidden)]
pub struct ToolDefinitionConst {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: &'static str,
}

impl ToolDefinition {
    /// Create a new tool definition from compile-time constants
    ///
    /// This is optimized for compile-time generated code where all strings
    /// are `'static`. The conversion to `ToolDefinition` happens at runtime
    /// but with zero allocation since we're just copying references.
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
    /// use tokitai_core::{ToolDefinition, ToolDefinitionConst};
    ///
    /// const TOOL_DATA: ToolDefinitionConst = ToolDefinitionConst {
    ///     name: "get_weather",
    ///     description: "Get weather information for a specified city",
    ///     input_schema: r#"{"type":"object"}"#,
    /// };
    ///
    /// let tool = ToolDefinition::from_const(TOOL_DATA);
    /// ```
    #[inline(always)]
    pub fn from_const(data: ToolDefinitionConst) -> Self {
        Self {
            #[cfg(feature = "serde")]
            name: data.name.into(),
            #[cfg(not(feature = "serde"))]
            name: data.name,
            #[cfg(feature = "serde")]
            description: data.description.into(),
            #[cfg(not(feature = "serde"))]
            description: data.description,
            #[cfg(feature = "serde")]
            input_schema: data.input_schema.into(),
            #[cfg(not(feature = "serde"))]
            input_schema: data.input_schema,
            version: None,
            deprecated_since: None,
            remove_in: None,
            replaced_by: None,
        }
    }

    /// Create a new tool definition (runtime version)
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
    #[cfg(feature = "serde")]
    pub fn new(
        name: impl Into<alloc::string::String>,
        description: impl Into<alloc::string::String>,
        input_schema: impl Into<alloc::string::String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: input_schema.into(),
            version: None,
            deprecated_since: None,
            remove_in: None,
            replaced_by: None,
        }
    }

    /// Create a new tool definition (no_std version)
    #[cfg(not(feature = "serde"))]
    pub fn new(
        name: &'static str,
        description: &'static str,
        input_schema: &'static str,
    ) -> Self {
        Self {
            name,
            description,
            input_schema,
            version: None,
            deprecated_since: None,
            remove_in: None,
            replaced_by: None,
        }
    }

    /// Set the tool version
    #[cfg(feature = "serde")]
    pub fn with_version(mut self, version: impl Into<alloc::string::String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the tool version (no_std version)
    #[cfg(not(feature = "serde"))]
    pub fn with_version(mut self, version: &'static str) -> Self {
        self.version = Some(version);
        self
    }

    /// Set deprecation information
    #[cfg(feature = "serde")]
    pub fn with_deprecated(
        mut self,
        deprecated_since: impl Into<alloc::string::String>,
        remove_in: impl Into<alloc::string::String>,
        replaced_by: impl Into<alloc::string::String>,
    ) -> Self {
        self.deprecated_since = Some(deprecated_since.into());
        self.remove_in = Some(remove_in.into());
        self.replaced_by = Some(replaced_by.into());
        self
    }

    /// Set deprecation information (no_std version)
    #[cfg(not(feature = "serde"))]
    pub fn with_deprecated(
        mut self,
        deprecated_since: &'static str,
        remove_in: &'static str,
        replaced_by: &'static str,
    ) -> Self {
        self.deprecated_since = Some(deprecated_since);
        self.remove_in = Some(remove_in);
        self.replaced_by = Some(replaced_by);
        self
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

    /// Get input schema as pretty-printed JSON string (requires `serde` feature)
    #[cfg(feature = "serde")]
    pub fn input_schema_pretty(&self) -> Result<String, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(&self.input_schema)?;
        serde_json::to_string_pretty(&value)
    }

    /// Get input schema as JSON Value (requires `serde` feature)
    #[cfg(feature = "serde")]
    pub fn input_schema_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.input_schema)
    }

    /// Apply configurations to this tool definition.
    ///
    /// This method is used by the configuration system to apply runtime
    /// configurations to tool definitions generated at compile time.
    ///
    /// # Parameters
    ///
    /// - `configs` - Slice of configuration items to apply
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::{ToolDefinition, ToolConfig};
    ///
    /// let mut tool = ToolDefinition::new("test", "Original description", "{}");
    /// tool.apply_configs(&[
    ///     ToolConfig::Desc("Overridden description".to_string()),
    /// ]);
    /// assert_eq!(tool.description, "Overridden description");
    /// ```
    #[cfg(feature = "serde")]
    pub fn apply_configs(&mut self, configs: &[ToolConfig]) {
        for config in configs {
            match config {
                ToolConfig::Desc(desc) => {
                    self.description = desc.clone();
                }
                ToolConfig::Tags(tags) => {
                    // Add tags to the schema
                    if let Ok(mut schema) =
                        serde_json::from_str::<serde_json::Value>(&self.input_schema)
                    {
                        if let Some(obj) = schema.as_object_mut() {
                            obj.insert("tags".to_string(), serde_json::json!(tags));
                        }
                        self.input_schema = schema.to_string();
                    }
                }
                ToolConfig::ParamDesc { name, desc } => {
                    self.apply_param_desc(name, desc);
                }
                ToolConfig::ParamExample { name, example } => {
                    self.apply_param_example(name, example);
                }
                ToolConfig::ParamDefault { name, default } => {
                    self.apply_param_default(name, default);
                }
                ToolConfig::ParamRequired { name, required } => {
                    self.apply_param_required(name, *required);
                }
                ToolConfig::ParamMin { name, min } => {
                    self.apply_param_constraint(name, "minimum", serde_json::json!(min));
                }
                ToolConfig::ParamMax { name, max } => {
                    self.apply_param_constraint(name, "maximum", serde_json::json!(max));
                }
                ToolConfig::ParamMinLength { name, min_length } => {
                    self.apply_param_constraint(name, "minLength", serde_json::json!(min_length));
                }
                ToolConfig::ParamMaxLength { name, max_length } => {
                    self.apply_param_constraint(name, "maxLength", serde_json::json!(max_length));
                }
                ToolConfig::ParamPattern { name, pattern } => {
                    self.apply_param_constraint(name, "pattern", serde_json::json!(pattern));
                }
                ToolConfig::ParamMinItems { name, min_items } => {
                    self.apply_param_constraint(name, "minItems", serde_json::json!(min_items));
                }
                ToolConfig::ParamMaxItems { name, max_items } => {
                    self.apply_param_constraint(name, "maxItems", serde_json::json!(max_items));
                }
                ToolConfig::ParamMultipleOf { name, multiple_of } => {
                    self.apply_param_constraint(name, "multipleOf", serde_json::json!(multiple_of));
                }
            }
        }
    }

    /// Apply parameter description to the schema.
    #[cfg(feature = "serde")]
    fn apply_param_desc(&mut self, name: &str, desc: &str) {
        if let Ok(mut schema) = serde_json::from_str::<serde_json::Value>(&self.input_schema) {
            if let Some(obj) = schema.as_object_mut() {
                if let Some(props) = obj.get_mut("properties").and_then(|v| v.as_object_mut()) {
                    if let Some(param) = props.get_mut(name).and_then(|v| v.as_object_mut()) {
                        param.insert("description".to_string(), serde_json::json!(desc));
                    }
                }
            }
            self.input_schema = schema.to_string();
        }
    }

    /// Apply parameter example to the schema.
    #[cfg(feature = "serde")]
    fn apply_param_example(&mut self, name: &str, example: &serde_json::Value) {
        if let Ok(mut schema) = serde_json::from_str::<serde_json::Value>(&self.input_schema) {
            if let Some(obj) = schema.as_object_mut() {
                if let Some(props) = obj.get_mut("properties").and_then(|v| v.as_object_mut()) {
                    if let Some(param) = props.get_mut(name).and_then(|v| v.as_object_mut()) {
                        param.insert("example".to_string(), example.clone());
                    }
                }
            }
            self.input_schema = schema.to_string();
        }
    }

    /// Apply parameter default to the schema.
    #[cfg(feature = "serde")]
    fn apply_param_default(&mut self, name: &str, default: &serde_json::Value) {
        if let Ok(mut schema) = serde_json::from_str::<serde_json::Value>(&self.input_schema) {
            if let Some(obj) = schema.as_object_mut() {
                if let Some(props) = obj.get_mut("properties").and_then(|v| v.as_object_mut()) {
                    if let Some(param) = props.get_mut(name).and_then(|v| v.as_object_mut()) {
                        param.insert("default".to_string(), default.clone());
                    }
                }
            }
            self.input_schema = schema.to_string();
        }
    }

    /// Apply parameter required flag to the schema.
    #[cfg(feature = "serde")]
    fn apply_param_required(&mut self, name: &str, required: bool) {
        if let Ok(mut schema) = serde_json::from_str::<serde_json::Value>(&self.input_schema) {
            if let Some(obj) = schema.as_object_mut() {
                // Update required array
                let required_arr = obj
                    .entry("required".to_string())
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut();

                if let Some(req_arr) = required_arr {
                    let name_json = serde_json::json!(name);
                    if required && !req_arr.contains(&name_json) {
                        req_arr.push(name_json);
                    } else if !required {
                        req_arr.retain(|v| v != &name_json);
                    }
                }

                // Also update parameter schema
                if let Some(props) = obj.get_mut("properties").and_then(|v| v.as_object_mut()) {
                    if let Some(param) = props.get_mut(name).and_then(|v| v.as_object_mut()) {
                        // Note: individual param "required" is not standard JSON Schema
                        // but we can add it for documentation purposes
                        param.insert("required".to_string(), serde_json::json!(required));
                    }
                }
            }
            self.input_schema = schema.to_string();
        }
    }

    /// Apply a constraint to a parameter in the schema.
    #[cfg(feature = "serde")]
    fn apply_param_constraint(
        &mut self,
        name: &str,
        constraint_key: &str,
        value: serde_json::Value,
    ) {
        if let Ok(mut schema) = serde_json::from_str::<serde_json::Value>(&self.input_schema) {
            if let Some(obj) = schema.as_object_mut() {
                if let Some(props) = obj.get_mut("properties").and_then(|v| v.as_object_mut()) {
                    if let Some(param) = props.get_mut(name).and_then(|v| v.as_object_mut()) {
                        param.insert(constraint_key.to_string(), value);
                    }
                }
            }
            self.input_schema = schema.to_string();
        }
    }
}

impl core::fmt::Display for ToolDefinition {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
            "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128"
            | "usize" | "isize" => Some(ParamType::Integer),
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
        Self {
            kind,
            message: message.into(),
        }
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
        Self::tool_definitions().iter().find(|t| t.name == name)
    }
}

/// # Tool Caller Trait
///
/// Provides runtime tool invocation capability.
/// Automatically implemented by the `#[tool]` macro for all tool types.
///
/// ## Example
///
/// ```rust,ignore
/// use tokitai_core::{ToolProvider, ToolCaller};
/// use serde_json::json;
///
/// // After using #[tool] macro on your type:
/// // struct Calculator;
/// // #[tool] impl Calculator { ... }
///
/// let calc = Calculator;
/// let result = calc.call_tool("add", &json!({"a": 10, "b": 20})).unwrap();
/// assert_eq!(result, json!(30));
/// ```
#[cfg(feature = "serde")]
pub trait ToolCaller {
    /// Call a tool by name with JSON arguments
    ///
    /// # Parameters
    ///
    /// - `name` - Tool name to call
    /// - `args` - JSON arguments for the tool
    ///
    /// # Returns
    ///
    /// - `Ok(Value)` - Tool execution result
    /// - `Err(ToolError)` - Tool execution failed
    fn call_tool(
        &self,
        name: &str,
        args: &crate::serde_types::Value,
    ) -> Result<crate::serde_types::Value, ToolError>;
}

/// # From Json Value Trait (P0 优化)
///
/// Trait for parsing JSON values into Rust types.
/// This trait is implemented for common types and used by the `#[tool]` macro
/// to parse tool parameters from JSON arguments.
///
/// ## Design Goals
///
/// - **Zero code duplication**: Implemented once per type, not per tool method
/// - **Compile-time monomorphization**: Generic over types for optimal performance
/// - **Clear error messages**: Type-specific error handling
///
/// ## Example
///
/// ```rust
/// use tokitai_core::{FromJsonValue, ToolError};
/// use serde_json::json;
///
/// let args = json!({"count": 42, "name": "test"});
/// let count = i64::from_json_value(&args, "count").unwrap();
/// let name = String::from_json_value(&args, "name").unwrap();
/// assert_eq!(count, 42);
/// assert_eq!(name, "test");
/// ```
#[cfg(feature = "serde")]
pub trait FromJsonValue: Sized {
    /// Parse a value from JSON arguments
    ///
    /// # Parameters
    ///
    /// - `args` - JSON arguments object
    /// - `key` - Parameter name to extract
    ///
    /// # Returns
    ///
    /// - `Ok(Self)` - Successfully parsed value
    /// - `Err(ToolError)` - Parsing failed (missing key or type mismatch)
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError>;

    /// Parse an optional value from JSON arguments
    ///
    /// # Parameters
    ///
    /// - `args` - JSON arguments object
    /// - `key` - Parameter name to extract
    ///
    /// # Returns
    ///
    /// - `Some(Self)` - Successfully parsed value
    /// - `None` - Key does not exist or type mismatch
    fn from_json_value_opt(args: &crate::serde_types::Value, key: &str) -> Option<Self> {
        Self::from_json_value(args, key).ok()
    }
}

// ============== 基本类型实现 ==============

#[cfg(feature = "serde")]
impl FromJsonValue for i64 {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("缺少必需参数 '{}'", key)))?
            .as_i64()
            .ok_or_else(|| ToolError::validation_error(format!("参数 '{}' 类型错误，期望 integer", key)))
    }
}

#[cfg(feature = "serde")]
impl FromJsonValue for i32 {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("缺少必需参数 '{}'", key)))?
            .as_i64()
            .map(|v| v as i32)
            .ok_or_else(|| ToolError::validation_error(format!("参数 '{}' 类型错误，期望 integer", key)))
    }
}

#[cfg(feature = "serde")]
impl FromJsonValue for u64 {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("缺少必需参数 '{}'", key)))?
            .as_u64()
            .ok_or_else(|| ToolError::validation_error(format!("参数 '{}' 类型错误，期望 unsigned integer", key)))
    }
}

#[cfg(feature = "serde")]
impl FromJsonValue for u32 {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("缺少必需参数 '{}'", key)))?
            .as_u64()
            .map(|v| v as u32)
            .ok_or_else(|| ToolError::validation_error(format!("参数 '{}' 类型错误，期望 unsigned integer", key)))
    }
}

#[cfg(feature = "serde")]
impl FromJsonValue for f64 {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("缺少必需参数 '{}'", key)))?
            .as_f64()
            .ok_or_else(|| ToolError::validation_error(format!("参数 '{}' 类型错误，期望 number", key)))
    }
}

#[cfg(feature = "serde")]
impl FromJsonValue for f32 {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("缺少必需参数 '{}'", key)))?
            .as_f64()
            .map(|v| v as f32)
            .ok_or_else(|| ToolError::validation_error(format!("参数 '{}' 类型错误，期望 number", key)))
    }
}

#[cfg(feature = "serde")]
impl FromJsonValue for bool {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("缺少必需参数 '{}'", key)))?
            .as_bool()
            .ok_or_else(|| ToolError::validation_error(format!("参数 '{}' 类型错误，期望 boolean", key)))
    }
}

#[cfg(feature = "serde")]
impl FromJsonValue for String {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("缺少必需参数 '{}'", key)))?
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ToolError::validation_error(format!("参数 '{}' 类型错误，期望 string", key)))
    }
}

// ============== &str 零拷贝支持 ==============
// 特殊处理：需要生命周期，使用单独函数
#[cfg(feature = "serde")]
#[inline(always)]
pub fn from_json_value_str<'a>(
    args: &'a crate::serde_types::Value,
    key: &str,
) -> Result<&'a str, ToolError> {
    args.get(key)
        .ok_or_else(|| ToolError::validation_error(format!("Missing required parameter '{}'", key)))?
        .as_str()
        .ok_or_else(|| ToolError::validation_error(format!("Parameter '{}' type error, expected string", key)))
}

// ============== Option 实现 ==============

#[cfg(feature = "serde")]
impl<T: FromJsonValue> FromJsonValue for Option<T> {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        Ok(T::from_json_value_opt(args, key))
    }
}

// ============== Vec 实现 ==============

#[cfg(feature = "serde")]
impl<T: serde::de::DeserializeOwned> FromJsonValue for Vec<T> {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        let value = args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("缺少必需参数 '{}'", key)))?;
        serde_json::from_value(value.clone())
            .map_err(|e| ToolError::validation_error(format!("参数 '{}' 类型错误：{}", key, e)))
    }
}

// ============== 辅助函数：解析任意 DeserializeOwned 类型 ==============
// 对于不支持的自定义类型，用户可以在方法内部手动反序列化

#[cfg(feature = "serde")]
#[inline(always)]
pub fn from_json_value_generic<T: serde::de::DeserializeOwned>(
    args: &crate::serde_types::Value,
    key: &str,
) -> Result<T, ToolError> {
    let value = args.get(key)
        .ok_or_else(|| ToolError::validation_error(format!("Missing required parameter '{}'", key)))?;
    serde_json::from_value(value.clone())
        .map_err(|e| ToolError::validation_error(format!("Parameter '{}' type error: {}", key, e)))
}

// ============== HashMap 实现 ==============
#[cfg(feature = "serde")]
impl<V: serde::de::DeserializeOwned> FromJsonValue for std::collections::HashMap<String, V> {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        let value = args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("Missing required parameter '{}'", key)))?;
        serde_json::from_value(value.clone())
            .map_err(|e| ToolError::validation_error(format!("Parameter '{}' type error: {}", key, e)))
    }
}

// ============== BTreeMap 实现 ==============
#[cfg(feature = "serde")]
impl<V: serde::de::DeserializeOwned> FromJsonValue for std::collections::BTreeMap<String, V> {
    #[inline(always)]
    fn from_json_value(args: &crate::serde_types::Value, key: &str) -> Result<Self, ToolError> {
        let value = args.get(key)
            .ok_or_else(|| ToolError::validation_error(format!("Missing required parameter '{}'", key)))?;
        serde_json::from_value(value.clone())
            .map_err(|e| ToolError::validation_error(format!("Parameter '{}' type error: {}", key, e)))
    }
}

/// Tool configuration types for runtime customization.
#[cfg(feature = "serde")]
pub mod config;

#[cfg(feature = "serde")]
pub mod serde_types {
    //! Serde type aliases
    //!
    //! This module is available when the `serde` feature is enabled.

    pub use alloc::string::String;
    pub use serde_json::Value;
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
        assert_eq!(
            ParamType::from_rust_type("Vec<i32>"),
            Some(ParamType::Array)
        );
    }

    #[test]
    fn test_tool_definition_const() {
        let tool = ToolDefinition::new("test", "A test tool", "{}");
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

// Async executor module (requires `async` feature)
#[cfg(feature = "async")]
pub mod executor;

#[cfg(feature = "async")]
pub use executor::{ExecutionError, ExecutionErrorKind, ToolExecutor, ExecutorStats};
