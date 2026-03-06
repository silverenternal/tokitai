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
//! tokitai = "0.3"
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
//!         Self::TOOL_DEFINITIONS
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
//! Rust types are automatically mapped to JSON Schema types:
//!
//! | Rust Type | JSON Schema Type |
//! |-----------|------------------|
//! | `String`, `str` | `string` |
//! | `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | `integer` |
//! | `f32`, `f64` | `number` |
//! | `bool` | `boolean` |
//! | `Vec<T>` | `array` |
//! | Custom structs | `object` |
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
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool::tool(attr, item)
}
