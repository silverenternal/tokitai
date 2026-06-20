# Tokitai MCP Server

[![Crates.io](https://img.shields.io/crates/v/tokitai-mcp-server.svg)](https://crates.io/crates/tokitai-mcp-server)
[![Documentation](https://docs.rs/tokitai-mcp-server/badge.svg)](https://docs.rs/tokitai-mcp-server)
[![License](https://img.shields.io/crates/l/tokitai-mcp-server)](../LICENSE)

## MCP Server Scaffolding

Tokitai MCP Server provides a server implementation built on the MCP (Model Context Protocol) specification, letting you stand up an AI-callable tool server in just a few lines of code.

## Core Features

- **Zero runtime overhead** — tool definitions are generated at compile time.
- **Type safety** — Rust's type system ensures AI-supplied arguments match your function signatures.
- **MCP compliant** — full support for the MCP protocol specification.
- **Easy to use** — boot a server in just a few lines of code.
- **HTTP support** — optional HTTP server with a RESTful API.

## Quick Start

### 1. Add the Dependencies

```toml
[dependencies]
tokitai = "0.6"
tokitai-mcp-server = "0.6"
tokio = { version = "1", features = ["full"] }
```

### 2. Define Your Tools

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
}
```

### 3. Build and Run the Server

```rust
use tokitai_mcp_server::McpServerBuilder;

#[tokio::main]
async fn main() {
    let server = McpServerBuilder::with_tool(Calculator::default())
        .with_port(8080)
        .build();

    server.run().await.unwrap();
}
```

### 4. Call the Server from an AI Client

```python
# Example Python MCP client
import requests

# Fetch the list of available tools
response = requests.get("http://127.0.0.1:8080/tools")
tools = response.json()

# Invoke a tool
response = requests.post(
    "http://127.0.0.1:8080/call",
    json={"name": "add", "arguments": {"a": 10, "b": 20}}
)
result = response.json()
print(result["result"])  # 30
```

## Core Types

| Type | Description |
|------|-------------|
| [`McpServer`] | The core MCP server type. |
| [`McpServerBuilder`] | A fluent builder used to construct a server. |
| [`MultiToolProvider`] | A multi-tool provider that aggregates several tool sets. |

## Using `MultiToolProvider`

```rust
use tokitai::tool;
use tokitai_mcp_server::MultiToolProvider;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[tool]
struct Greeter;

#[tool]
impl Greeter {
    pub fn greet(&self, name: String) -> String {
        format!("Hello, {}!", name)
    }
}

// Compose several tool sets into one provider
let mut provider = MultiToolProvider::new();
provider.add(Calculator::default());
provider.add(Greeter::default());
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/tools` | GET | Return the definitions of every available tool. |
| `/call`  | POST | Invoke a named tool with a JSON arguments object. |
| `/health` | GET | Liveness check endpoint. |

## Configuration Options

Use `McpServerBuilder` to configure the server:

```rust
let server = McpServerBuilder::with_tool(my_tool)
    .with_port(8080)           // set the listening port
    .with_host("0.0.0.0")      // set the bind address
    .build();
```

## Features

| Feature | Description |
|---------|-------------|
| `default` | Default configuration. |

## Requirements

- **Rust version**: 1.80+
- **Edition**: 2021

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE](../LICENSE))
- MIT License ([LICENSE](../LICENSE))

at your option.

## Related Crates

| Crate | Crates.io | Description |
|-------|-----------|-------------|
| `tokitai` | [![crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai) | Main crate, bundling runtime support. |
| `tokitai-core` | [![crates.io](https://img.shields.io/crates/v/tokitai-core.svg)](https://crates.io/crates/tokitai-core) | Core types and traits. |
| `tokitai-macros` | [![crates.io](https://img.shields.io/crates/v/tokitai-macros.svg)](https://crates.io/crates/tokitai-macros) | Procedural macro implementation. |

## Documentation

- **[API Reference](https://docs.rs/tokitai-mcp-server)** — complete API documentation.
- **[Quick Start](../docs/quickstart.md)** — five-minute getting-started tutorial.
- **[MCP Architecture](../docs/MCP_ARCHITECTURE.md)** — notes on the MCP protocol design.

---

**Happy Coding!**
