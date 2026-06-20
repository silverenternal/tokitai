# Tokitai

> **Current version: 0.5.0** (released 2026-06-02). See [CHANGELOG](CHANGELOG.md#050---2026-06-02) and the [v0.4 to v0.5 migration guide](docs/migration/v0.4-to-v0.5.md).

[![Crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai)
[![Documentation](https://docs.rs/tokitai/badge.svg)](https://docs.rs/tokitai)
[![License](https://img.shields.io/crates/l/tokitai)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/silverenternal/tokitai/ci.yml)](https://github.com/silverenternal/tokitai/actions)

> **Design philosophy**: Tokitai is fundamentally an **in-process** tool-calling library. It generates type-safe `__call_*` wrapper functions at compile time, and `call_tool` dispatches them directly inside your Rust process's memory — **no network, no IPC round-trip, no serialization back to `serde_json::Value`**. Wire protocols like MCP, HTTP, and stdio are simply **optional out-of-process wrappers** built on top of that core, not the core itself.

## One attribute, and your Rust code is AI-callable

```rust
use tokitai::tool;

#[tool]  // <- this is the only line you need!
impl MyTools {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

**Compile-time generation** · **Zero runtime intrusion** · **Type-safe by construction**

---

**Compile-time AI tool definitions · Minimal runtime footprint · Magic-sticker integration**

Tokitai is a procedural-macro library. A single `#[tool]` attribute turns your Rust methods into AI-callable tools. Every tool definition is generated at compile time, so type errors surface during compilation rather than at runtime. The runtime surface stays minimal (serde + serde_json) with no extra overhead.

## 5-minute quick start

### 1. Add the dependency

```toml
[dependencies]
tokitai = "0.6"
tokitai-mcp-server = "0.6"  # optional: MCP server scaffolding
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
```

That's it. All required dependencies (serde, serde_json, thiserror) are pulled in automatically.

### 2. Define your tools

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

### 3. Get the tool definitions

```rust
let tools = Calculator::tool_definitions();  // v0.4.0+ returns a method, not a constant
```

### 4. Handle an AI call

```rust
use tokitai::json;

let calc = Calculator::default();
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}))?;
println!("{}", result);  // 30
```

### 5. Spin up an MCP HTTP server in seconds

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

Then call it from any MCP client:

```python
import requests

# List available tools
response = requests.get("http://127.0.0.1:8080/tools")
tools = response.json()

# Call a tool
response = requests.post(
    "http://127.0.0.1:8080/call",
    json={"name": "add", "arguments": {"a": 10, "b": 20}}
)
result = response.json()
print(result["result"])  # 30
```

### Run the examples

```bash
# Basic usage
cargo run --example basic_usage

# MCP server demo
cargo run --example mcp_builder_demo -p tokitai-mcp-server

# Multi-tool chat
cargo run --example multi_tool_chat

# End-to-end regression test
cargo run --example dev_assistant
```

## Documentation

- **[5-minute quick start](docs/quickstart.md)** — a more detailed getting-started walkthrough
- **[Advanced usage](docs/ADVANCED_USAGE.md)** — advanced features and best practices
- **[Type system](docs/USAGE.md)** — how Rust types map to JSON Schema
- **[AI integration](docs/AI_INTEGRATION.md)** — integrating with AI providers
- **[Architecture](docs/ARCHITECTURE.md)** — project structure and design
- **[Wrap architecture](docs/wrap-architecture.md)** — the auto-wrapping macros: `#[wrap]`, `#[openapi]`, `#[delegate]`, `#[retry]`, and friends
- **[Wrap cheatsheet](docs/wrap-cheatsheet.md)** — one-page reference for the Wrap family
- **[Cross-language SDK guide](docs/CROSS_LANGUAGE.md)** — the HTTP+JSON protocol plus quickstarts for Python, JS/TS, Go, and curl
- **[API reference](https://docs.rs/tokitai)** — full API documentation

## Core features

| Feature | Description |
|---------|-------------|
| **Minimal dependency footprint** | Add only `tokitai = "0.6"`; the runtime needs just serde + serde_json |
| **Compile-time generation** | Tool definitions are generated during compilation, so type errors surface early |
| **One attribute** | Just `#[tool]` — no chain of annotations to remember |
| **Type-safe by construction** | Rust types are mapped to JSON Schema automatically |
| **Provider-agnostic** | Works with any AI / LLM provider |
| **Compile-time dialect correctness** | `#[tool(dialect = "openai-strict")]` (or `"anthropic"` / `"mcp"`) audits the emitted JSON Schema against the chosen provider's known quirks and refuses to compile on a violation. No more "works in Claude, fails in OpenAI" surprises at runtime. See [§8 Dialect correctness](docs/wrap-architecture.md#8-dialect-correctness). |

## Wrap features (v0.5+)

In addition to the core `#[tool]` macro, Tokitai ships a family of **auto-wrapping** macros that turn existing clients, OpenAPI specs, and resilience policies into tools with a single attribute:

| Macro | Purpose |
|-------|---------|
| `#[wrap]` | Whitelist-pick methods from a third-party client and generate a `new(client)` constructor |
| `#[openapi]` / `#[openapi_op]` | Read an OpenAPI 3 spec and expose the matching `operationId`s as a batch of HTTP tools |
| `#[delegate]` | Forward inner methods as tools without writing a hand-rolled `match` dispatcher |
| `#[retry]` | Inject an exponential-backoff retry loop inside the tool body |
| `#[rate_limit]` | Apply a lock-free token-bucket rate limiter before the tool body runs |
| `#[circuit_breaker]` | Three-state circuit breaker; v1 is observe-only and does not yet trip |

For the full picture see [Wrap architecture](docs/wrap-architecture.md) and the [Wrap cheatsheet](docs/wrap-cheatsheet.md).
Per-macro argument lists live under `docs/reference/` (one file per macro).

## Type mapping

| Rust type | JSON Schema |
|-----------|-------------|
| `String`, `&str` | `string` |
| `i32`, `i64`, `u32`, ... | `integer` |
| `f32`, `f64` | `number` |
| `bool` | `boolean` |
| `Vec<T>` | `array` |
| `Option<T>` | optional `T` |
| user-defined `struct` | `object` |

## Common attributes

```rust
#[tool]
impl MyTools {
    /// Custom name
    #[tool(name = "custom_name")]
    pub fn my_func(&self) {}

    /// Custom description
    #[tool(desc = "Custom description")]
    pub fn another_func(&self) {}

    /// Per-parameter attributes
    pub fn process(
        &self,
        #[tool(desc = "Parameter description", default = "null")]
        options: Option<String>
    ) {}
}
```

For the full attribute reference see [Advanced usage](docs/ADVANCED_USAGE.md).

## Performance

| Operation | Cost |
|-----------|------|
| Macro compile time | < 50 ms |
| Tool definition generation | Zero runtime cost (done at compile time) |
| `call_tool` invocation | < 1 μs |

> Benchmark environment: Rust 1.75, M1 Pro, 16 GB RAM.
>
> Run the benchmarks with `cargo bench --bench macro_bench`.

## Project layout

Tokitai is shipped as three crates:

| Crate | Role |
|-------|------|
| `tokitai` | Main crate, includes the runtime support |
| `tokitai-core` | Core types and traits (zero dependencies) |
| `tokitai-macros` | Procedural-macro implementation |

**For 99% of users, this is all you need:**

```toml
[dependencies]
tokitai = "0.5.0"
```

## Requirements

- **Rust version**: 1.80+
- **Edition**: 2021

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE](LICENSE))
- MIT License ([LICENSE](LICENSE))

at your option.

## Contributing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual-licensed as above, without any additional terms or conditions.

## Examples

More examples live under the [examples directory](examples/):

- `basic_usage.rs` — basic usage
- `advanced_types.rs` — advanced types and features, end to end
- `mcp_server_demo.rs` — MCP server example
- `mcp_http_server.rs` — HTTP server example
- `ollama_integration.rs` — Ollama AI integration
- `dev_assistant.rs` — end-to-end integration: file / code search + Git + calculator (downstream regression test since v0.5)
- `multi_tool_chat.rs` — multi-tool chat
- `param_attrs.rs` — per-parameter attributes
- `validate_transform_alias.rs` — validation / transformation / aliases
- `debug_tools.rs` — debugging utilities
- `wrap_demo.rs` — `#[tool]` + `MultiToolProvider` composition pattern that `#[wrap]` / `#[delegate]` are designed to wrap (runnable today; see `examples/deprecated/README.md` for the attribute export schedule)
- `runtime_agnostic.rs` — runtime-agnostic async executor bridge
- `database_tool/` — realistic example: Tokitai + MCP HTTP + SQLite (sqlx)
- `starter_project/` — copy-paste-ready starter template

> The `#[wrap]` / `#[delegate]` / `#[retry]` / `#[rate_limit]` /
> `#[circuit_breaker]` / `#[openapi]` attributes are implemented in
> `tokitai-macros` but are **not yet exposed** through the public
> `tokitai` / `tokitai_macros` re-exports as of 0.5.x. The
> [`examples/deprecated/`](examples/deprecated/) directory
> contains only a `README.md` with a tracking-issue table for
> each attribute. The reference docs under `docs/wrap-architecture.md`
> describe the planned public API.

### Cross-language SDK (HTTP+JSON client references)

The HTTP+JSON protocol served by `tokitai-mcp-server` is callable from any language. Reference implementations:

- Python — [`examples/py/`](examples/py/) — async client on `httpx`; `pip install -e .`
- JavaScript / TypeScript — [`examples/js/`](examples/js/) — zero-runtime-dep `fetch` client for Node 18+, browsers, Deno, Bun; `npm install && npm start`
- Go — [`examples/go/`](examples/go/) — std-lib only; `go build ./...`, `go run ./cmd/list-tools`
- `curl` — [`examples/curl/`](examples/curl/) — `bash` + `curl` + `jq`; zero install, great for CI

Start the server in a separate terminal with
`cargo run -p tokitai-mcp-server --example mcp_builder_demo` (binds
`http://127.0.0.1:8080`); the SDKs above will talk to it out of the
box. Override the host with `BASE_URL` (curl), an env var (Go), or the
constructor argument (Python, JS). Full protocol spec and per-language
quickstarts in [Cross-language SDK guide](docs/CROSS_LANGUAGE.md).

## API stability

Tokitai follows [Semantic Versioning](https://semver.org/). For the full stability policy see [API stability](docs/API_STABILITY.md).

**Current status**: the v0.5.x series — core API is stable, v1.0.0 is on the roadmap.

---

**Happy coding!**
