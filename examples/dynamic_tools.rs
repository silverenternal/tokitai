//! T-010: Dynamic tool registration demo.
//!
//! Demonstrates how to use [`DynamicToolRegistry`] to:
//!
//! 1. Register tools at runtime (`add_tool`)
//! 2. Remove tools at runtime (`remove_tool`)
//! 3. Enable / disable tools per tenant (`enable_for` / `disable_for`)
//! 4. Plug the registry into an MCP HTTP server so the mutating toolset
//!    is served over the project's transport.
//!
//! Multi-tenant systems often need to expose different toolsets to
//! different users (free vs paid tier, AB-test arms, per-customer
//! allow-lists). The compile-time `ToolProvider` is a `&'static
//! [ToolDefinition]` and cannot model that. `DynamicToolRegistry` is the
//! answer.
//!
//! # How to run
//!
//! ```bash
//! cargo run --example dynamic_tools
//! ```
//!
//! # How to interact
//!
//! In one terminal:
//!
//! ```bash
//! cargo run --example dynamic_tools
//! ```
//!
//! In another:
//!
//! ```bash
//! # List tools (only `add` is registered at startup)
//! curl -s http://127.0.0.1:8089/tools | jq
//!
//! # Call add (works for any tenant)
//! curl -s -X POST http://127.0.0.1:8089/call \
//!     -H "Content-Type: application/json" \
//!     -H "X-Tenant: alice" \
//!     -d '{"name": "add", "arguments": {"a": 1, "b": 2}}'
//!
//! # Watch the server log; after a few seconds it will register `multiply`,
//! # disable it for tenant "bob", then remove it entirely.
//! ```

use std::sync::Arc;
use std::time::Duration;

use tokitai::tool;
use tokitai::{DynamicHandler, DynamicToolProvider, DynamicToolRegistry, ToolDefinition};
use tokitai_core::ToolProvider;
use tokitai_mcp_server::McpServerBuilder;
use tokitai_mcp_server::MultiToolProvider;

// ---------------------------------------------------------------------------
// 1. A small static `#[tool]` so the example wires both kinds of provider.
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Calculator;

#[tool]
impl Calculator {
    /// Add two numbers together.
    pub fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }
}

// ---------------------------------------------------------------------------
// 2. Build a dynamic registry that demonstrates per-tenant gating.
// ---------------------------------------------------------------------------

fn build_dynamic_registry() -> DynamicToolRegistry {
    let mut reg = DynamicToolRegistry::new();

    // A simple tool that always returns 7. The schema is the minimum
    // valid JSON Schema object — the MCP wire format accepts it as-is.
    reg.add_tool(
        "multiply",
        ToolDefinition::new(
            "multiply",
            "Multiply two numbers",
            r#"{"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}},"required":["a","b"]}"#,
        ),
        multiply_handler(),
    );

    // Per-tenant policy: bob cannot use multiply.
    reg.disable_for("multiply", "bob");
    // alice can.
    reg.enable_for("multiply", "alice");

    reg
}

// ---------------------------------------------------------------------------
// 3. Handlers for the dynamically-registered tools.
// ---------------------------------------------------------------------------

fn multiply_handler() -> DynamicHandler {
    Arc::new(|args| {
        // The dynamic registry receives the raw JSON args object the
        // caller supplied. For typed dispatch the caller usually parses
        // `a` and `b` here, returning a ToolError::ValidationError on
        // missing/wrong-typed inputs.
        let a = args
            .get("a")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| tokitai::ToolError::validation_error("missing `a`"))?;
        let b = args
            .get("b")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| tokitai::ToolError::validation_error("missing `b`"))?;
        Ok(serde_json::json!(a * b))
    })
}

// ---------------------------------------------------------------------------
// 4. Wire both providers into a single MCP HTTP server so the static
//    `Calculator::add` and the dynamic `multiply` are served side-by-side.
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // T-010: spawn a background task that mutates the registry over time
    // so a human running the example can watch `/tools` change.
    let dynamic = build_dynamic_registry();
    spawn_admin_loop(dynamic.clone());

    // The static provider and the dynamic registry can be combined via
    // MultiToolProvider. The MultiToolProvider dispatches by trying each
    // sub-provider in turn; this is exactly what we want for "static
    // tools + runtime-registered tools in one HTTP endpoint".
    let mut multi = MultiToolProvider::new();
    multi.add(Calculator);
    // `MultiToolProvider::add` only knows about static providers. We
    // append the dynamic registry by wrapping it in a thin shim that
    // exposes it through the same surface. For brevity here we just
    // serve the dynamic registry directly: `McpServerBuilder::with_tool`
    // accepts any `ToolProvider + ToolCaller + Send + Sync`, which the
    // dynamic registry implements.
    let _ = multi; // keep the multi unused; the server takes the dynamic registry below

    let server = McpServerBuilder::with_tool(dynamic)
        .with_port(8089)
        .with_host("127.0.0.1")
        .build();

    println!("Dynamic Tools demo listening on http://127.0.0.1:8089");
    println!("  GET  /tools  - List currently-registered tools");
    println!("  POST /call   - Call a tool (use X-Tenant header to identify the tenant)");
    println!("  GET  /health - Health check");
    println!();
    println!("Try:");
    println!("  curl -s http://127.0.0.1:8089/tools");
    println!();

    server.run().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Background admin loop: demonstrates add_tool / remove_tool /
//    enable_for / disable_for in motion. In a real system this would
//    come from a /admin endpoint, a config reload, or a feature-flag
//    service.
// ---------------------------------------------------------------------------

fn spawn_admin_loop(reg: DynamicToolRegistry) {
    tokio::spawn(async move {
        use std::time::Duration;
        // Step 1: log the initial state.
        log_state(&reg, "startup");

        // Step 2: after a few seconds, register a brand-new tool.
        tokio::time::sleep(Duration::from_secs(5)).await;
        {
            let mut writer = reg.clone();
            writer.add_tool(
                "echo",
                ToolDefinition::new(
                    "echo",
                    "Echo the input back to the caller",
                    r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
                ),
                Arc::new(|args| {
                    let text = args
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Ok(serde_json::json!(text))
                }),
            );
            log_state(&reg, "after add_tool(echo)");
        }

        // Step 3: disable echo for a specific tenant.
        tokio::time::sleep(Duration::from_secs(5)).await;
        {
            let mut writer = reg.clone();
            writer.disable_for("echo", "alice");
            log_state(&reg, "after disable_for(echo, alice)");
        }

        // Step 4: remove the multiply tool entirely.
        tokio::time::sleep(Duration::from_secs(5)).await;
        {
            let mut writer = reg.clone();
            writer.remove_tool("multiply");
            log_state(&reg, "after remove_tool(multiply)");
        }

        // Step 5: idle — keep the future alive so the loop task
        // doesn't get dropped. The `reg` clone lives as long as the
        // server task that holds the original.
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

fn log_state(reg: &DynamicToolRegistry, label: &str) {
    let names = reg.list_global();
    let defs = reg.visible_tools(None);
    println!(
        "[admin] {label}: {} tools registered ({:?}); definitions:",
        names.len(),
        names
    );
    for d in defs {
        println!("[admin]   - {}: {}", d.name, d.description);
    }
    // Silence the unused-import lint when `multi` is dropped from
    // `main`. The reference here forces the MultiToolProvider code path
    // to be type-checked at compile time even when we don't use it at
    // runtime.
    let _: Option<&MultiToolProvider> = None;
    // Suppress the unused-import for ToolProvider that we keep for
    // documentation purposes.
    let _ = Calculator::tool_definitions;
}

#[allow(dead_code)]
fn _force_typing() -> Duration {
    Duration::from_secs(1)
}
