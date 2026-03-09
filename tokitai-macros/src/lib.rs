//! # Tokitai Macros
//!
//! **Procedural macros for Tokitai - Zero-dependency macro for AI tool integration**
//!
//! This crate provides the `#[tool]` procedural macro that enables compile-time tool definitions
//! for AI/LLM tool calling systems. It generates all the boilerplate code needed to expose
//! your Rust functions as AI-callable tools.
//!
//! ## 🎯 Key Features
//!
//! - **Zero Runtime Dependencies** - The macro itself has no runtime overhead
//! - **Compile-time Generation** - Tool definitions are generated at compile time
//! - **Type Safety** - Parameter validation happens at compile time
//! - **Automatic Discovery** - Mark an `impl` block and all `pub` methods become tools
//! - **Customizable** - Override tool names and descriptions via attributes
//! - **Vendor Neutral** - Works with any AI/LLM provider (Ollama, OpenAI, Anthropic, etc.)
//!
//! ## Quick Start
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tokitai = "0.3.3"
//! ```
//!
//! Then use the `#[tool]` macro:
//!
//! ```rust,ignore
//! use tokitai::tool;
//!
//! pub struct Calculator;
//!
//! #[tool]
//! impl Calculator {
//!     /// Add two numbers together
//!     pub async fn add(&self, a: i32, b: i32) -> i32 {
//!         a + b
//!     }
//!
//!     /// Multiply two numbers
//!     pub async fn multiply(&self, a: i32, b: i32) -> i32 {
//!         a * b
//!     }
//! }
//!
//! // Usage
//! let calc = Calculator;
//!
//! // Get tool definitions (compile-time generated)
//! let tools = Calculator::tool_definitions();
//! println!("Number of tools: {}", tools.len());
//!
//! // Call a tool
//! let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20})).unwrap();
//! println!("Result: {}", result);  // 30
//! ```
//!
//! ## How It Works
//!
//! The `#[tool]` macro automatically:
//!
//! 1. Extracts doc comments as tool descriptions
//! 2. Generates JSON Schema for parameters from Rust types
//! 3. Creates `TOOL_DEFINITIONS` constant with all tool metadata
//! 4. Implements `call_tool` dispatcher for runtime invocation
//! 5. Generates parameter parsing and validation code
//!
//! ## Customization
//!
//! You can customize tool names and descriptions using the attribute syntax:
//!
//! ```rust,ignore
//! #[tool]
//! impl MyTools {
//!     #[tool(name = "fetch_url", desc = "Fetch content from a URL")]
//!     pub async fn fetch(&self, url: String) -> String {
//!         // implementation
//!     }
//! }
//! ```
//!
//! ## Generated Code
//!
//! For each `#[tool]` impl block, the macro generates:
//!
//! ```rust,ignore
//! // 1. Tool definitions constant
//! impl Calculator {
//!     pub const TOOL_DEFINITIONS: &'static [ToolDefinition] = &[
//!         ToolDefinition {
//!             name: "add",
//!             description: "Add two numbers together",
//!             input_schema: "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}",
//!         },
//!         // ... more tools
//!     ];
//! }
//!
//! // 2. ToolProvider trait implementation
//! impl ToolProvider for Calculator {
//!     fn tool_definitions() -> &'static [ToolDefinition] {
//!         Self::tool_definitions()
//!     }
//! }
//!
//! // 3. call_tool dispatcher
//! impl Calculator {
//!     pub fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
//!         match name {
//!             "add" => self.__call_add(args).await,
//!             "multiply" => self.__call_multiply(args).await,
//!             _ => Err(ToolError::not_found(format!("Unknown tool: {}", name))),
//!         }
//!     }
//! }
//! ```
//!
//! ## Type Mapping
//!
//! Rust types are automatically mapped to JSON Schema types with full recursive support:
//!
//! ### Basic Types
//!
//! | Rust Type | JSON Schema Type |
//! |-----------|------------------|
//! | `String`, `str` | `string` |
//! | `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `isize` | `integer` |
//! | `f32`, `f64` | `number` |
//! | `bool` | `boolean` |
//!
//! ### Compound Types
//!
//! | Rust Type | JSON Schema Type |
//! |-----------|------------------|
//! | `Vec<T>` | `array` with `items` schema |
//! | `[T; N]` | `array` with `items` schema |
//! | `&[T]` | `array` with `items` schema |
//! | `HashMap<K, V>` | `object` with `additionalProperties` |
//! | `Option<T>` | `anyOf` with inner type and `null` |
//! | `(T, U, ...)` | `array` (tuple representation) |
//!
//! ### Third-party Types
//!
//! | Rust Type | JSON Schema Type |
//! |-----------|------------------|
//! | `chrono::DateTime<Utc>` | `string` with `format: date-time` |
//! | `chrono::NaiveDateTime` | `string` with `format: date-time` |
//! | `uuid::Uuid` | `string` with `format: uuid` |
//! | `url::Url` | `string` with `format: uri` |
//! | `PathBuf`, `Path` | `string` with `format: file-path` |
//!
//! ### Custom Types
//!
//! Custom structs are represented as `object` types. For full field-level schema generation,
//! ensure your struct derives `serde::Deserialize` and the macro will handle it at runtime.
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
//! - [`tokitai-core`](https://crates.io/crates/tokitai-core) - Core types and traits

mod tool;

use proc_macro::TokenStream;

/// # `#[tool]` Attribute Macro
///
/// Marks an `impl` block or individual methods as AI-callable tools.
///
/// ## Usage
///
/// ### 1. Mark an impl block (Recommended)
///
/// When placed on an `impl` block, all `pub` methods are automatically registered as tools:
///
/// ```rust,ignore
/// pub struct Calculator;
///
/// #[tool]
/// impl Calculator {
///     /// Add two numbers together
///     pub async fn add(&self, a: i32, b: i32) -> i32 {
///         a + b
///     }
/// }
/// ```
///
/// ### 2. Mark individual methods
///
/// Use `#[tool(...)]` on specific methods to customize tool properties:
///
/// ```rust,ignore
/// #[tool]
/// impl Calculator {
///     #[tool(name = "add_numbers", desc = "Add two numbers together")]
///     pub async fn add(&self, a: i32, b: i32) -> i32 {
///         a + b
///     }
///
///     /// This method won't be registered as a tool
///     fn helper(&self) {}
/// }
/// ```
///
/// ### 3. Mark individual parameters
///
/// Use `#[tool_attr(...)]` on specific parameters to customize parameter properties:
///
/// ```rust,ignore
/// #[tool]
/// impl Calculator {
///     pub async fn add(
///         &self,
///         #[tool_attr(required)] a: Option<i32>,  // Option but required
///         #[tool_attr(default = "0")] b: Option<i32>,  // Has default value
///     ) -> i32 {
///         a.unwrap_or(0) + b.unwrap_or(0)
///     }
/// }
/// ```
///
/// ## Generated Code
///
/// The macro generates:
///
/// 1. `const TOOL_DEFINITIONS: &'static [ToolDefinition]` - Compile-time tool definitions
/// 2. `fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError>` - Tool dispatcher
/// 3. Wrapper functions for each tool with JSON parameter parsing
///
/// ## Features
///
/// - ✅ Zero runtime dependencies (macro itself)
/// - ✅ Compile-time tool definition generation
/// - ✅ Automatic description extraction from doc comments
/// - ✅ Custom tool names and descriptions support
/// - ✅ Type-safe parameter parsing
/// - ✅ Recursive type resolution for complex types
/// - ✅ JSON Schema generation with proper formatting
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool::tool(attr, item)
}

/// # `#[tool_type]` Attribute Macro
///
/// Registers a custom type with a manually defined JSON schema.
///
/// Use this macro when you have custom struct types that the `#[tool]` macro cannot automatically parse.
/// This allows you to provide explicit schema information for AI tool calling.
///
/// ## Usage
///
/// ```rust,ignore
/// use tokitai::tool_type;
///
/// #[tool_type(
///     name = "Location",
///     properties = "latitude: number, longitude: number",
///     required = "latitude, longitude"
/// )]
/// pub struct Location {
///     pub latitude: f64,
///     pub longitude: f64,
/// }
/// ```
///
/// ## Attributes
///
/// - `name`: The type name (required)
/// - `properties`: Comma-separated list of `field_name: type` pairs
///   - Supported types: `string`, `integer`, `number`, `boolean`, `array`, `object`
/// - `required`: Comma-separated list of required field names
///
/// ## Generated Schema
///
/// The above example generates:
///
/// ```json
/// {
///   "type": "object",
///   "properties": {
///     "latitude": { "type": "number" },
///     "longitude": { "type": "number" }
///   },
///   "required": ["latitude", "longitude"]
/// }
/// ```
#[proc_macro_attribute]
pub fn tool_type(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool::tool_type(attr, item)
}

/// Parameter validation attribute (used internally by #[tool] macro)
///
/// This attribute is automatically processed by the `#[tool]` macro.
/// It should not be used directly by users - instead use the `#[tool(...)]` syntax on parameters.
///
/// ## Example (internal usage)
///
/// ```text
/// #[tool]
/// impl MyTools {
///     pub fn create_user(
///         &self,
///         #[tool_validate = "!value.is_empty()")]
///         name: String,
///     ) -> Result<String, Error> {
///         // ...
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn tool_validate(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // This is a no-op attribute, processed by #[tool] macro
    item
}

/// Parameter transformation attribute (used internally by #[tool] macro)
///
/// This attribute is automatically processed by the `#[tool]` macro.
/// It should not be used directly by users - instead use the `#[tool(...)]` syntax on parameters.
///
/// ## Example (internal usage)
///
/// ```text
/// #[tool]
/// impl MyTools {
///     pub fn create_user(
///         &self,
///         #[tool_transform = "value.to_lowercase()")]
///         email: String,
///     ) -> Result<String, Error> {
///         // ...
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn tool_transform(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // This is a no-op attribute, processed by #[tool] macro
    item
}

/// Parameter description attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_desc(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter example attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_example(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter default attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_default(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter required attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_required(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter min attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_min(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter max attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_max(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter min_length attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_min_length(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter max_length attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_max_length(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter pattern attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_pattern(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter min_items attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_min_items(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter max_items attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_max_items(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter multiple_of attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_multiple_of(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter-level tool attributes (helper macro for #[tool])
///
/// This attribute is used to add validation, transformation, and other metadata
/// to individual parameters. It is automatically processed by the `#[tool]` macro.
///
/// ## Usage
///
/// ```rust,ignore
/// #[tool]
/// impl MyTools {
///     pub fn create_user(
///         &self,
///         #[param_tool(validate = "!value.is_empty()", desc = "Name cannot be empty")]
///         name: String,
///         #[param_tool(transform = "value.to_lowercase()")]
///         email: String,
///         #[param_tool(default = 10)]
///         count: i32,
///     ) -> Result<String, Error> {
///         // ...
///     }
/// }
/// ```
///
/// ## Supported Attributes
///
/// - `validate = "expression"` - Validation expression (use `value` to refer to the parameter)
/// - `transform = "expression"` - Transformation expression (use `value` to refer to the parameter)
/// - `desc = "description"` - Parameter description
/// - `default = value` - Default value (supports literals: integers, floats, booleans, strings, arrays, objects)
/// - `example = value` - Example value
/// - `required` - Mark parameter as required (even if Option type)
#[proc_macro_attribute]
pub fn param_tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // This is a no-op attribute, processed by #[tool] macro
    item
}

/// # `tokitai!` Configuration Macro
///
/// Used to centrally configure tool properties without modifying original code.
///
/// ## Usage
///
/// ```rust,ignore
/// tokitai::config! {
///     MyService {
///         get_user: {
///             desc: "获取用户信息",
///             tags: ["user", "read"],
///             params: {
///                 id: {
///                     desc: "用户唯一标识",
///                     example: "1001"
///                 }
///             }
///         }
///     }
/// }
/// ```
///
/// ## Features
///
/// - Override method descriptions
/// - Add tags to methods
/// - Configure parameter-level descriptions and examples
/// - Works with existing `#[tool]` annotated code
///
/// ## Priority
///
/// 1. `#[tool(desc = "...")]` > doc comments
/// 2. `tokitai!` config > `#[tool]` attributes
/// 3. Parameter-level: `#[param_tool]` > default inference
#[proc_macro]
pub fn config(item: TokenStream) -> TokenStream {
    tool::config(item)
}
