//! Tokitai MCP Server - Build AI-callable tool servers with zero runtime overhead
//!
//! This crate provides scaffolding for building MCP (Model Context Protocol) servers
//! using Tokitai's compile-time tool definitions.
//!
//! ## Features
//!
//! - **Zero Runtime Overhead** - Tool definitions generated at compile time
//! - **Type Safe** - Rust's type system ensures AI parameters match your functions
//! - **MCP Compliant** - Full support for MCP protocol
//! - **Easy to Use** - Get started with just a few lines of code
//!
//! ## Quick Start
//!
//! ### 1. Define your tools
//!
//! ```rust,ignore
//! use tokitai::tool;
//!
//! #[tool]
//! struct Calculator;
//!
//! #[tool]
//! impl Calculator {
//!     /// Add two numbers together
//!     pub fn add(&self, a: i32, b: i32) -> i32 {
//!         a + b
//!     }
//! }
//! ```
//!
//! ### 2. Create and run the server
//!
//! ```rust,ignore
//! use tokitai_mcp_server::McpServerBuilder;
//!
//! #[tokio::main]
//! async fn main() {
//!     let server = McpServerBuilder::with_tool(Calculator::default())
//!         .with_port(8080)
//!         .build();
//!
//!     server.run().await.unwrap();
//! }
//! ```
//!
//! ### 3. Call from AI client
//!
//! ```python
//! # Python MCP client example
//! import requests
//!
//! # Get available tools
//! response = requests.get("http://127.0.0.1:8080/tools")
//! tools = response.json()
//!
//! # Call a tool
//! response = requests.post("http://127.0.0.1:8080/call", json={
//!     "name": "add",
//!     "arguments": {"a": 10, "b": 20}
//! })
//! result = response.json()
//! print(result["result"])  # 30
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐         ┌─────────────────────┐         ┌─────────────────┐
//! │   AI Client     │         │  MCP Server         │         │  Business Logic │
//! │   (Python/JS)   │ ──────> │  (tokitai-mcp)      │ ──────> │  (Rust tools)   │
//! │                 │ <────── │                     │ <────── │  #[tool]        │
//! └─────────────────┘         └─────────────────────┘         └─────────────────┘
//!      │                           │                              │
//!      │ 1. List tools             │                              │
//!      │ 2. Call tool (JSON)       │                              │
//!      │                           │ 3. Type-safe call            │
//!      │                           │                              │
//!      │ 4. Result (JSON)          │                              │
//! ```
//!
//! ## Features
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `default` | Default features |
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
//! - Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE))
//! - MIT License ([LICENSE-MIT](../LICENSE))
//!
//! at your option.

pub mod server;
pub mod stdio;
// T-024: runtime cross-crate version assertion. `serve()` is the
// top-level CLI entry point that reads `--require-tokitai=<prefix>`
// and `--allow-tokitai-mismatch` from `std::env::args`, then
// enforces the rule against a build-script-emitted manifest
// (`OUT_DIR/tokitai_manifest.rs`). See `serve.rs` for the CLI
// contract and the documented hard-refusal / soft-warn split.
pub mod serve;

// T-021: typed MCP handle layer. Compiled unconditionally (it has no
// dependency on rmcp or any MCP SDK), but the typed dispatch path is
// wired in only when the consumer enables the `mcp-typed` feature.
// With the feature off, this module is exposed but unused; with the
// feature on, the `call_tool` dispatch path validates the caller's
// arguments against the fixture's `inputSchema` BEFORE the handler
// runs. See `docs/MCP_ARCHITECTURE.md` § "Typed handle layer (T-021)".
pub mod typed;

pub use typed::{
    load_typed_fixtures, validate_against_schema, validate_tool_args, JsonPointer, TypedDispatcher,
    TypedToolSpec,
};

// T-024: re-export the runtime cross-crate version entry point.
// Downstream binaries wire `tokitai_mcp_server::serve()` into their
// `fn main()` to gate startup on the version check.
///
/// `run_version_check` is a documented alias for [`serve`] - the
/// name is more descriptive at the call site (e.g. a build script
/// or a `tokio::main` that wires several guards). It re-exports
/// the same function pointer; calling either name is equivalent.
/// Prefer `serve()` in new code for symmetry with the rest of
/// the crate's entry-point naming.
pub use serve::serve as run_version_check;

// Re-export commonly used types
pub use server::{
    McpServer, McpServerBuilder, McpServerConfig, MultiToolProvider, ServerError, ToolCallerDyn,
};
pub use stdio::StdioServer;

/// T-023: convenience helper that takes a `&[&str]` allowlist
/// (the canonical literal form operators write in deployment
/// configs) and returns a `Vec<String>` the builder can
/// consume. The conversion is a no-op clone: each `&str` is
/// converted to a `String`. The function is the documented
/// entry point for operators who do not want to construct the
/// `Vec<String>` by hand at every call site.
///
/// # Example
///
/// ```rust,ignore
/// use tokitai_mcp_server::{serve_with_manifest, McpServerBuilder};
///
/// let allowlist = serve_with_manifest(&["db:read:*", "net:egress:smtp"]);
/// let server = McpServerBuilder::with_tool(MyTools::default())
///     .with_capability_allowlist(allowlist)
///     .build();
/// ```
pub fn serve_with_manifest(allowlist: &[&str]) -> Vec<String> {
    allowlist.iter().map(|s| s.to_string()).collect()
}

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
