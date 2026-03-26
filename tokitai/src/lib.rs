//! # Tokitai
//!
//! **AI Tool Integration System with Compile-time Tool Definitions**
//!
//! Tokitai is a zero-runtime-dependency procedural macro library that transforms your Rust methods
//! into AI-callable tools with a single `#[tool]` attribute. All tool definitions are generated at
//! compile time, ensuring type errors are caught before runtime.
//!
//! ## 🎯 Key Features
//!
//! - **Zero Runtime Intrusion** - The macro itself has no runtime dependencies
//! - **Compile-time Type Safety** - Tool definitions generated at compile time, parameter type errors exposed during compilation
//! - **Single Attribute** - Just `#[tool]`, no need for multiple tags
//! - **Optional Runtime** - Control dependencies via features, supports async-free environments
//! - **Vendor Neutral** - Works with any AI/LLM provider (Ollama, OpenAI, Anthropic, etc.)

#![allow(unexpected_cfgs)]

//! ## 🚀 Quick Start
//!
//! ### 1. Add Dependencies
//!
//! ```toml
//! [dependencies]
//! tokitai = "0.3.3"
//! ```
//!
//! That's it! All required dependencies (serde, serde_json, thiserror) are included automatically.
//!
//! ### 2. Define Your Tools
//!
//! ```rust,ignore
//! use tokitai::tool;
//!
//! pub struct Calculator;
//!
//! #[tool]
//! impl Calculator {
//!     /// Add two numbers together
//!     pub fn add(&self, a: i32, b: i32) -> i32 {
//!         a + b
//!     }
//!
//!     /// Calculate SHA256 hash of a string
//!     pub fn sha256(&self, input: String) -> String {
//!         // Your implementation...
//!         format!("hash of {}", input)
//!     }
//! }
//! ```
//!
//! ### 3. Get Tool Definitions (Send to AI)
//!
//! ```rust,ignore
//! // Compile-time generated tool definitions
//! let tools = Calculator::tool_definitions();
//!
//! // Convert to JSON and send to AI
//! let tools_json = serde_json::to_string_pretty(tools)?;
//! println!("{}", tools_json);
//! ```
//!
//! Output:
//!
//! ```json
//! [
//!   {
//!     "name": "add",
//!     "description": "Add two numbers together",
//!     "input_schema": "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}"
//!   },
//!   {
//!     "name": "sha256",
//!     "description": "Calculate SHA256 hash of a string",
//!     "input_schema": "{\"type\":\"object\",\"properties\":{\"input\":{\"type\":\"string\"}},\"required\":[\"input\"]}"
//!   }
//! ]
//! ```
//!
//! ### 4. Handle AI Calls
//!
//! ```rust,ignore
//! use tokitai::json;
//!
//! let calc = Calculator;
//!
//! // AI decides to call a tool
//! let call_request = json!({
//!     "name": "add",
//!     "arguments": {"a": 10, "b": 20}
//! });
//!
//! // Execute the tool
//! let result = calc.call_tool(
//!     call_request["name"].as_str().unwrap(),
//!     &call_request["arguments"]
//! )?;
//!
//! println!("Result: {}", result);  // 30
//! ```
//!
//! ## 📦 Crate Structure
//!
//! Tokitai is organized as a workspace with three crates:
//!
//! | Crate | Description |
//! |-------|-------------|
//! | [`tokitai`](https://crates.io/crates/tokitai) | Main crate with runtime support (this crate) |
//! | [`tokitai-core`](https://crates.io/crates/tokitai-core) | Core types and traits (zero dependencies) |
//! | [`tokitai-macros`](https://crates.io/crates/tokitai-macros) | Procedural macros (compile-time code generation) |
//!
//! ## 🔧 How It Works
//!
//! ```text
//! +---------------+    Tool Definitions    +---------------+
//! |  Your Code    | ----------------------> |  AI Service   |
//! |  #[tool]      |                         |  (Ollama,     |
//! +---------------+                         |   OpenAI,     |
//!       ^                                   |   etc.)       |
//!       | Execution Result                  +---------------+
//!       |                                         |
//!       |                                         | Call Request
//!       |                                         v
//! +---------------+                         +---------------+
//! |  Rust Method  | <------ call_tool ------ |  JSON Call    |
//! |  (Local)      |                         | {"name":..}   |
//! +---------------+                         +---------------+
//! ```
//!
//! 1. **Define Rust methods** → Implement your business logic
//! 2. **Send to AI** → AI knows what tools are available
//! 3. **Receive call request** → AI returns "I want to call a tool"
//! 4. **Execute and return** → Run Rust code locally
//!
//! ## 🛠️ Features
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `default` | Enables full runtime support |
//! | `runtime` | Basic runtime support (async, error handling) |
//! | `mcp` | MCP protocol support (requires `runtime`) |
//!
//! ### Minimal Dependencies (Compile-time Only)
//!
//! If you only need compile-time tool definitions without runtime support:
//!
//! ```toml
//! [dependencies]
//! tokitai = { version = "0.3", default-features = false }
//! ```
//!
//! Note: Runtime features (call_tool, etc.) require serde/serde_json which are included by default.
//!
//! ## 📚 API Overview
//!
//! ### Re-exported Core Types
//!
//! - [`ToolDefinition`] - Tool definition with name, description, and input schema
//! - [`ToolError`] - Tool invocation error type
//! - [`ToolErrorKind`] - Error classification
//! - [`ParamType`] - JSON Schema type enumeration
//! - [`ToolProvider`] - Trait for tool providers (auto-implemented by `#[tool]`)
//! - [`json!`] - Macro for creating JSON values (from serde_json)
//! - [`Value`], [`Map`] - JSON value types (from serde_json)
//!
//! ### Runtime Types
//!
//! - [`AiToolError`] - Enhanced error type for runtime
//!
//! ### Macro
//!
//! - [`tool`] - Attribute macro for marking tool implementations
//!
//! ## 📋 Type Mapping
//!
//! Rust types are automatically mapped to JSON Schema types:
//!
//! | Rust Type | JSON Schema Type |
//! |-----------|------------------|
//! | `String`, `&str` | `string` |
//! | `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | `integer` |
//! | `f32`, `f64` | `number` |
//! | `bool` | `boolean` |
//! | `Vec<T>` | `array` |
//! | Custom structs | `object` |
//!
//! ## 📖 Examples
//!
//! See the [examples directory](https://github.com/silverenternal/tokitai/tree/main/examples) for more:
//!
//! - `basic_usage.rs` - Basic usage example
//! - `ollama_integration.rs` - Ollama AI integration with SHA256 tool
//! - `multi_tool_chat.rs` - Multi-tool协作 chatbot
//!
//! ## ⚙️ Requirements
//!
//! - **Rust Version**: 1.70+
//! - **Edition**: 2021
//!
//! ## 📄 License
//!
//! Licensed under either of:
//!
//! - Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/silverenternal/tokitai/blob/main/LICENSE))
//! - MIT License ([LICENSE-MIT](https://github.com/silverenternal/tokitai/blob/main/LICENSE))
//!
//! at your option.
//!
//! ## 🤝 Contributing
//!
//! Unless you explicitly state otherwise, any contribution intentionally submitted
//! for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be
//! dual licensed as above, without any additional terms or conditions.
//!
//! ## See Also
//!
//! - [`tokitai-core`](https://crates.io/crates/tokitai-core) - Core types and traits
//! - [`tokitai-macros`](https://crates.io/crates/tokitai-macros) - Procedural macros

// Re-export core types (always available)
pub use tokitai_core::{
    ParamType, ToolCaller, ToolDefinition, ToolError, ToolErrorKind, ToolProvider,
};

// Re-export serde_json for convenience (users don't need to add extra dependency)
pub use serde_json::{json, Map, Value};

// Re-export config types (when serde feature is enabled)
#[cfg(feature = "serde")]
pub use tokitai_core::{ToolConfig, ToolConfigRegistry, GLOBAL_CONFIG_REGISTRY};

// Runtime module (always available)
pub mod error;

#[cfg(feature = "mcp")]
pub mod mcp;

// Export runtime types
pub use error::AiToolError;

#[cfg(feature = "mcp")]
pub use mcp::*;

// Re-export macros
pub use tokitai_macros::{
    config, param_tool, tool, tool_default, tool_desc, tool_example, tool_max, tool_max_items,
    tool_max_length, tool_min, tool_min_items, tool_min_length, tool_multiple_of, tool_pattern,
    tool_required, tool_transform, tool_type, tool_validate,
};

// Async executor (requires `async` feature)
#[cfg(feature = "async")]
pub use tokitai_core::executor;

#[cfg(feature = "async")]
pub use tokitai_core::{ExecutionError, ExecutionErrorKind, ToolExecutor, ExecutorStats};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
