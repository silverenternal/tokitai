# Tokitai x MCP Architecture Guide

**Version**: 0.5.0
**Last updated**: 2026-06-02

---

## Overview

This guide explains how to use Tokitai to build AI tool servers based on MCP (Model Context Protocol).

### Core philosophy

> **Compile-time generation. Zero runtime intrusion. Type safety.**

Tokitai's design philosophy lines up naturally with the MCP protocol, making Rust the best language for writing AI-native backends.

---

## Architecture

### End-to-end architecture

```
+-----------------+         +---------------------+         +-----------------+
|   AI Client     |         |  MCP Server         |         |  Business Logic |
|   (Python/JS)   | ------> |  (tokitai-mcp)      | ------> |  (Rust tools)   |
|                 | <------ |                     | <------ |  #[tool]        |
+-----------------+         +---------------------+         +-----------------+
     |                           |                              |
     | 1. List tools             |                              |
     | 2. Call tool (JSON)       |                              |
     |                           | 3. Type-safe call            |
     |                           |                              |
     | 4. Result (JSON)          |                              |
```

### Components

| Component | Role | Key technology |
|-----------|------|----------------|
| **AI Client** | Lightweight decision-maker | Sends JSON requests only; never loads business code |
| **MCP Server** | Compile-time processing hub | Tokitai macros generate the tool definitions |
| **Business Logic** | Strongly typed core | Rust code marked with `#[tool]` |

---

## Quickstart

### 1. Add the dependency

```toml
[dependencies]
tokitai = { version = "0.5.0", features = ["mcp"] }
tokitai-mcp-server = "0.5"  # Optional: MCP server scaffolding
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
```

### 2. Define a tool

```rust
use tokitai::tool;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    /// Add two numbers
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Compute the SHA-256 hash of the input
    pub fn sha256(&self, input: String) -> String {
        // Your business logic
        format!("hash of {}", input)
    }
}
```

### 3. Get the tool definitions

```rust
// Compile-time generated tool definitions
let tools = Calculator::tool_definitions();

// Convert to the MCP format
let mcp_tools = tokitai::mcp::to_mcp_tools(&tools);

// Send to the AI
let tools_json = serde_json::to_string_pretty(&mcp_tools)?;
```

### 4. Handle a tool call

```rust
use serde_json::json;

let calc = Calculator::default();

// The AI decides to call a tool
let call_request = json!({
    "name": "add",
    "arguments": {"a": 10, "b": 20}
});

// Execute the tool (type-safe)
let result = calc.call_tool(
    call_request["name"].as_str().unwrap(),
    &call_request["arguments"]
)?;

println!("{}", result);  // 30
```

---

## Full example: an MCP HTTP server

### Using the `tokitai-mcp-server` scaffolding

```rust
use tokitai::tool;
use tokitai_mcp_server::McpServerBuilder;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[tokio::main]
async fn main() {
    let server = McpServerBuilder::with_tool(Calculator::default())
        .with_port(8080)
        .build();

    server.run().await.unwrap();
}
```

### Custom HTTP server

```rust
use axum::{routing::{get, post}, Json, Router};
use tokitai::{tool, mcp::to_mcp_tools, ToolProvider};
use serde_json::json;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[tokio::main]
async fn main() {
    // Get the tool definitions
    let tools = to_mcp_tools(&Calculator::tool_definitions());

    // Build the routes
    let app = Router::new()
        .route("/tools", get(|| async { tools }))
        .route("/call", post(|body: Json<Value>| async {
            // Handle the tool call
            json!({"success": true, "result": 30})
        }));

    // Start the server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

---

## Running the examples

### Example 1: basic MCP server demo

```bash
# Run from the project root
cargo run --example mcp_server_demo
```

Sample output:

```
=== Tokitai MCP Server Example ===

Loaded tools:
  - add: Add two numbers
  - multiply: Multiply two numbers
  - sqrt: Compute the square root
  - sha256: Compute the SHA-256 hash of a string
  - get_weather: Get weather information for a given city
  - get_current_time: Get the current date and time

MCP tool definitions (6 total):
  - add (150 bytes)
  - multiply (140 bytes)
  ...

=== Tool-call demo ===

[1] Math
   [tool call] add({"a": 100, "b": 250})
    add(100, 250) = 350
```

### Example 2: HTTP server

```bash
# Start the HTTP server
cargo run --example mcp_http_server

# In another terminal, exercise the API
# List tools
curl http://127.0.0.1:8080/tools

# Call a tool
curl -X POST http://127.0.0.1:8080/call \
  -H "Content-Type: application/json" \
  -d '{"name": "add", "arguments": {"a": 10, "b": 20}}'
```

---

## AI client integration

### Python example

```python
import requests

# List tools
response = requests.get("http://127.0.0.1:8080/tools")
tools = response.json()
print(f"Available tools: {len(tools)}")

# Call a tool
response = requests.post(
    "http://127.0.0.1:8080/call",
    json={"name": "add", "arguments": {"a": 10, "b": 20}}
)
result = response.json()
print(f"Result: {result['result']}")  # 30
```

### JavaScript example

```javascript
// List tools
const toolsResponse = await fetch('http://127.0.0.1:8080/tools');
const tools = await toolsResponse.json();

// Call a tool
const callResponse = await fetch('http://127.0.0.1:8080/call', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    name: 'add',
    arguments: { a: 10, b: 20 }
  })
});
const result = await callResponse.json();
console.log(`Result: ${result.result}`);  // 30
```

---

## Architectural advantages

### 1. Lightweight

| Layer | Advantage |
|-------|-----------|
| **Agent side** | No business code, minimal context |
| **Transport** | JSON serialization, minimal payload size |
| **Runtime** | No interpreter overhead, native execution |

### 2. Strong compile-time guarantees

| Property | How it works |
|----------|--------------|
| **Schema generation** | Procedural macros generate schemas at compile time; no runtime reflection |
| **Type checking** | The Rust type system guarantees argument matching |
| **Error capture** | Type errors are caught at compile time |

### 3. MCP flexibility

| Capability | Notes |
|------------|-------|
| **Language-agnostic** | The agent can be Python, JavaScript, or anything else |
| **Standard protocol** | Follows the MCP specification |
| **Extensible** | New tools are easy to add |

### 4. Wrapper macros for production hardening (v0.5.0)

Tokitai v0.5.0 ships wrapper macros that decorate the generated dispatcher with cross-cutting behavior. They are especially valuable on the MCP server side, where one bad call can stall the whole agent loop.

| Macro | Why it matters on an MCP server |
|-------|----------------------------------|
| `#[wrap]` | Inject custom middleware (logging, metrics, request/response transformation) around `call_tool` |
| `#[openapi]` | Emit an OpenAPI / Swagger document directly from the tool surface, so the MCP server is self-describing |
| `#[delegate]` | Forward a subset of tools to a sub-provider, enabling per-tenant or per-namespace routing |
| `#[retry]` | Retry transient failures with backoff before propagating to the AI |
| `#[rate_limit]` | Throttle per-tool or globally to protect the server from runaway agents |
| `#[circuit_breaker]` | Trip the circuit after repeated failures so the server can shed load |

These macros stack: apply several of them to one impl block and the macro pipeline resolves their order. For most MCP servers you will want at least `#[retry]`, `#[rate_limit]`, and `#[circuit_breaker]`.

---

## Type mapping

| Rust type | JSON Schema | Example |
|-----------|-------------|---------|
| `String`, `&str` | `string` | `"hello"` |
| `i32`, `i64`, `u32` | `integer` | `42` |
| `f32`, `f64` | `number` | `3.14` |
| `bool` | `boolean` | `true` |
| `Vec<T>` | `array` | `[1, 2, 3]` |
| Custom struct | `object` | `{"name": "Alice"}` |

---

## Type-safety guarantees

### Compile-time checks

```rust
#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// If the AI sends a wrong argument type, call_tool returns a runtime error.
// But the Rust type system has already guaranteed the function signature is correct.
```

### Runtime validation

```rust
let calc = Calculator::default();

// A parameter type mismatch returns an error
let result = calc.call_tool("add", &json!({
    "a": "not a number",  // wrong: should be an integer
    "b": 20
}));

assert!(result.is_err());
```

---

## Best practices

### 1. Tool design

```rust
#[tool]
impl MyTools {
    /// Clear doc comment (automatically becomes the tool description for the AI)
    #[tool(tags = ["category", "feature"])]
    pub fn process_data(
        &self,
        #[param_tool(desc = "Input data", example = "sample")]
        input: String,

        #[param_tool(desc = "Processing options", default = "null")]
        options: Option<Vec<String>>,
    ) -> Result<String, MyError> {
        // Business logic
    }
}
```

### 2. Error handling

```rust
use tokitai::AiToolError;

#[tool]
impl MyTools {
    pub fn risky_operation(&self, data: String) -> Result<String, AiToolError> {
        // Validate input
        if data.is_empty() {
            return Err(AiToolError::validation_error("Data cannot be empty"));
        }

        // Business logic
        Ok(format!("Processed: {}", data))
    }
}
```

### 3. Performance

```rust
// Prefer: default-derived types
#[derive(Default, Clone)]
struct MyTools;

// Prefer: reuse a single instance
let tools = MyTools::default();
let result1 = tools.call_tool("op1", &args1)?;
let result2 = tools.call_tool("op2", &args2)?;
```

---

## Related resources

- [5-minute quickstart](quickstart.md)
- [Advanced usage](ADVANCED_USAGE.md)
- [Type system](USAGE.md)
- [AI integration](AI_INTEGRATION.md)
- [API documentation](https://docs.rs/tokitai)

---

## FAQ

### Q: Do I have to use `tokitai-mcp-server` for an MCP server?

A: No. `tokitai-mcp-server` is an optional scaffolding crate; you can build the server with any HTTP framework you like (axum, actix-web, and so on).

### Q: How do I add tools at runtime?

A: Tokitai generates tool definitions at compile time, so it does not support runtime registration. If you need dynamic tools, compose multiple `#[tool]` types.

### Q: Are async tools supported?

A: Yes. With the `runtime` feature enabled, you can define `async fn` tools.

```rust
#[tool]
impl AsyncTools {
    pub async fn fetch_url(&self, url: String) -> String {
        reqwest::get(&url).await.unwrap().text().await.unwrap()
    }
}
```

---

## Stdio transport (pinned to MCP `2025-06-18`)

`tokitai-mcp-server` ships a **stdio** MCP transport that lets any
`#[tool]` provider speak to clients like Claude Desktop, Cursor, and the
official Python MCP SDK over newline-delimited JSON-RPC.

```rust
use tokitai_mcp_server::{McpServerBuilder, MultiToolProvider};

let mut provider = MultiToolProvider::new();
provider.add(MyTools);

let stdio = McpServerBuilder::with_tool(provider).with_stdio();
stdio.serve().await?;
```

Run with:

```bash
cargo run --example mcp_stdio_server -p tokitai-mcp-server
```

Hand-drive a session (each line is one JSON-RPC frame):

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  | cargo run --example mcp_stdio_server -p tokitai-mcp-server
```

### Supported methods

| Method | Direction | Response |
|--------|-----------|----------|
| `initialize` | client → server | serverInfo + capabilities |
| `ping` | client → server | `{}` |
| `tools/list` | client → server | array of compiled `McpTool`s |
| `tools/call` | client → server | `{content:[{type:"text",text:"..."}],isError:false}` |
| `notifications/*` | client → server | (no response, silently ignored) |
| anything else | client → server | JSON-RPC `MethodNotFound` (`-32601`) |

### Re-syncing the hand-rolled framer when the MCP spec revs

The stdio transport deliberately does **not** depend on `rmcp` or any
other MCP SDK. The framer is pinned to MCP `2025-06-18` and lives in
`tokitai-mcp-server/src/stdio.rs` (one ~440-line module). A small
fixture mirrors the spec at `tokitai-mcp-server/tests/fixtures/mcp-spec/`:

- `protocol-version.txt` — current MCP spec tag (e.g. `2025-06-18`)
- `samples/initialize.request.json` — pinned `initialize` request shape
- `samples/initialize.response.json` — pinned `initialize` response shape
- `README.md` — re-sync procedure

When the MCP spec revs, the procedure is:

1. Bump `protocol-version.txt` and update `MCP_PROTOCOL_VERSION` in
   `src/stdio.rs`.
2. Add or modify `match` arms in `handle_request` for any new methods
   the spec requires for tools (`tools/list`, `tools/call`,
   `notifications/...`). Drop methods that are removed.
3. Add or refresh the sample JSON files in `samples/`.
4. Re-run the smoke test (`cargo test -p tokitai-mcp-server --test
   mcp_stdio_smoke`) and bless any snapshot drift deliberately.

No upstream SDK is touched. Spec conformance is re-established by a
single PR.
