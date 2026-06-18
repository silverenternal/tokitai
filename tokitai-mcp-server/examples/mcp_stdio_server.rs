//! MCP stdio server example
//!
//! Runs the `tokitai-mcp-server` JSON-RPC-over-stdio transport against
//! the official MCP `2025-06-18` spec. Suitable for connecting from
//! Claude Desktop, Cursor, or the official Python MCP SDK via the
//! `mcp-client` stdio transport.
//!
//! # How to run
//!
//! ```bash
//! # From the project root
//! cargo run --example mcp_stdio_server -p tokitai-mcp-server
//! ```
//!
//! # Connecting from the official Python SDK
//!
//! ```python
//! import asyncio
//! from mcp import ClientSession, StdioServerParameters
//! from mcp.client.stdio import stdio_client
//!
//! async def main():
//!     params = StdioServerParameters(
//!         command="cargo",
//!         args=["run", "--example", "mcp_stdio_server", "-p", "tokitai-mcp-server"],
//!     )
//!     async with stdio_client(params) as (read, write):
//!         async with ClientSession(read, write) as session:
//!             await session.initialize()
//!             tools = await session.list_tools()
//!             print([t.name for t in tools.tools])
//!             result = await session.call_tool("add", {"a": 10, "b": 20})
//!             print(result)
//!
//! asyncio.run(main())
//! ```
//!
//! # Hand-driving the protocol (for debugging)
//!
//! ```bash
//! # In one terminal:
//! cargo run --example mcp_stdio_server -p tokitai-mcp-server
//!
//! # In another, pipe JSON-RPC frames at it (one per line):
//! echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
//!   | cargo run --example mcp_stdio_server -p tokitai-mcp-server
//! ```

use tokitai::tool;
use tokitai_mcp_server::{McpServerBuilder, MultiToolProvider};

/// Tiny calculator for the demo.
#[derive(Default, Clone)]
pub struct Calculator;

#[tool]
impl Calculator {
    /// Add two integers
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Multiply two integers
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }

    /// Reverse a string
    pub fn reverse(&self, text: String) -> String {
        text.chars().rev().collect()
    }
}

fn main() -> std::io::Result<()> {
    // Tokio runtime — required because the stdio transport is async.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        // Build a multi-tool provider so we can show more than one tool.
        let mut provider = MultiToolProvider::new();
        provider.add(Calculator);

        let server = McpServerBuilder::with_tool(provider).with_stdio();
        server.serve().await
    })
}
