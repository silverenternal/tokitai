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
tokitai = { version = "0.6", features = ["mcp"] }
tokitai-mcp-server = "0.6"  # Optional: MCP server scaffolding
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

---

## In-process tool-call tracing (T-015)

Tokitai's `#[tool]` macro can splice a `#[tracing::instrument(...)]`
span into every generated `__call_*` wrapper. The intent is to
make every tool call observable by default, with **zero cost
when the feature is off** — no separate sidecar process, no
JSON-RPC traffic sniffer (such as `mcpdog` or Wireshark), no
runtime plugin to load.

### Why this exists

Sidecar traffic sniffers exist because MCP and similar
JSON-RPC protocols are *opaque*: you cannot tell which tool
was called, with what arguments, or how long it took without
running a process between the client and server. For in-process
Tokitai deployments the equivalent should be a compile-time
flag that emits structured spans around the call site — zero
cost when disabled, full structured trace when enabled, and
no separate sidecar required. Compile-time injection is the
only way to deliver that: Python/Java runtimes can't get free
observability because their call paths aren't expanded at
compile time.

### Enabling the trace feature

```toml
[dependencies]
tokitai = { version = "0.6", features = ["trace"] }
```

Or via a compile-time env var (no Cargo.toml change):

```bash
TOKITAI_TRACE=1 cargo build
```

`tokitai-macros/build.rs` forwards the env var into the
macro's compile environment; the macro reads it via
`option_env!("TOKITAI_TRACE")` and emits the
`#[tracing::instrument(...)]` attribute on every wrapper.
The macro re-exports `tokitai::tracing` so consumers do not
need a separate `tracing = "0.1"` dep in their Cargo.toml.

### What gets recorded

Each call emits one span named `tokitai_tool_call` carrying:

| Field          | Type   | Source                                                       |
|----------------|--------|--------------------------------------------------------------|
| `tool.name`    | string | `#[tool]` primary name (or `#[tool(name = "...")]` override) |
| `tool.version` | string | `#[tool(version = "...")]` or `"-"` when unset               |
| `args.size`    | u64    | byte length of the JSON arguments object                     |
| `result.size`  | u64    | byte length of the JSON result object (0 on error)           |

The `result.size` field is recorded on both the success and
error arms so subscribers can filter on it unconditionally
(matching the four-keys-always-present contract used by
`tokitai-mcp-server`'s HTTP middleware).

### Wiring a subscriber

```rust
use tracing_subscriber::EnvFilter;

let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

let provider = MyTools;
let result = provider.call_tool("add", &serde_json::json!({"a": 2, "b": 40}))?;
// One span: tokitai_tool_call{tool.name="add", tool.version="1.2.0",
//                              args.size=21, result.size=2}
```

Or run `dev_assistant.rs` directly:

```bash
RUST_LOG=tokitai=trace cargo run --example dev_assistant
```

### Zero-cost default

When the `trace` feature is off (the default) and
`TOKITAI_TRACE` is unset, the macro emits no `tracing`
references anywhere in the generated code. The binary-size
smoke test in CI verifies the stripped binary is byte-
identical modulo a single `tracing::Span::current()`
reference inside an `if false { ... }` branch that the
linker drops. The hot path of `call_tool` therefore compiles
to the same machine code with or without the feature.

### Comparison with sidecar traffic sniffers

| Approach                    | Setup cost       | Runtime cost (off) | Runtime cost (on)   | Captures      |
|-----------------------------|------------------|---------------------|----------------------|---------------|
| `mcpdog`-style sidecar      | external process | always-on overhead  | always-on overhead   | JSON-RPC only |
| Wireshark                   | external tool    | capture overhead    | capture overhead     | bytes on wire |
| Tokio `tracing-subscriber`  | log crate        | none                | span-emit cost       | call site     |
| **Tokitai `trace` feature** | compile flag     | **none**            | span-emit cost       | call site     |

The Tokitai feature is unique in the "runtime cost (off)" cell:
because the spans are spliced in at compile time, a build with
the feature off literally does not contain the calls.

## Typed handle layer (T-021)

The typed MCP handle layer is the project's defense-in-depth against
the CVE-2025-59377 class of MCP vulnerabilities — tools whose
handlers accept an unvalidated JSON string and concatenate it into a
shell command (`subprocess.run(..., shell=True)`), `eval`, or any
other string sink. The architectural flaw is not in MCP itself: the
JSON schema is advertised in `tools/list`, but the handler never
enforces it. The schema validation happens AFTER the sink, which
defeats the point of having a schema at all.

T-021 inverts that ordering. Before the call reaches the handler,
the typed layer validates every argument against the tool's
`inputSchema` (read from the fixture in
`tests/fixtures/mcp-spec/typed/*.json`). If validation fails, the
handler is never invoked; the caller receives
`ToolError::ValidationError` with a JSON Pointer (RFC 6901) to the
offending field in the error message. No shell. No `eval`. No
subprocess.

### Threat model

The typed layer catches:

| Class                              | Example                                              |
|------------------------------------|------------------------------------------------------|
| Wrong JSON type for a property     | `a: "ten"` where the schema says `integer`           |
| Missing required property          | `{ "a": 1 }` where `b` is required                   |
| Extra property under `additionalProperties: false` | `{ "a": 1, "b": 2, "injected": "; rm -rf /" }` |
| Numeric out-of-range               | `user_id: 0` where `minimum: 1`                      |
| String length out-of-range         | 100-char `title` where `maxLength: 64`               |
| Wrong root type                    | `arguments: [1,2,3]` where schema says `object`      |

It does **not** catch:

- Semantic validation (a value the schema accepts but the business
  logic rejects).
- Side-channel validation (handler-internal state).
- Authorization (handled above this layer).

### Feature gate

The typed layer is enabled by the **`mcp-typed`** feature in
`tokitai-mcp-server`'s `Cargo.toml`. It is **off by default**:

```toml
# tokitai-mcp-server/Cargo.toml
[features]
default = []
mcp-typed = []   # opt-in: typed handle layer
```

```bash
# Default build (T-005 JSON-passthrough path; behavior unchanged):
cargo build -p tokitai-mcp-server

# Opt in to the typed layer:
cargo build -p tokitai-mcp-server --features mcp-typed
```

With the feature off, the typed module is compiled but unused; the
wire-level transport behaves identically to the T-005 path. With
the feature on, every `tools/call` validates the caller's arguments
against the fixture's `inputSchema` before the handler runs.

### Architecture

```
+----------------+   tools/call    +----------------------+   validated    +---------------+
|  AI client     | -------------> |  typed layer (T-021) | -------------> |   handler     |
|  (LLM-driven)  |   (JSON args)  |  (validates against  |   (typed args) |   (your code) |
|                | <------------- |   fixture's schema)  | <------------ |               |
+----------------+   ValidationError   +----------------------+   ToolError   +---------------+
                                    refused before
                                    handler runs
```

### CVE-2025-59377 → T-021 mapping

| CVE-2025-59377 vulnerability              | T-021 mitigation                                  |
|-------------------------------------------|---------------------------------------------------|
| `subprocess.run(..., shell=True)` sink    | Typed layer refuses malformed calls before the    |
|                                           | handler is constructed. No subprocess is spawned. |
| Validation, if any, runs AFTER the sink   | Validation runs BEFORE the handler is called.     |
| Type confusion (`str` for `int`) is a     | Type confusion is caught at the typed boundary.  |
| silent success                            | The handler never sees the malformed input.       |
| Shell-metacharacter injection via `kubectl` | Argument shape mismatch is rejected at the JSON   |
| argument                                  | Pointer `/spec.command`.                          |

### Public API

```rust
use tokitai_mcp_server::typed::{
    JsonPointer, TypedDispatcher, TypedToolSpec,
    load_typed_fixtures, validate_against_schema, validate_tool_args,
};

// Direct schema validation:
let schema = serde_json::json!({
    "type": "object",
    "properties": { "a": { "type": "integer" } },
    "required": ["a"],
    "additionalProperties": false,
});
validate_against_schema(&schema, &serde_json::json!({"a": "ten"}))?;
// Err(ToolError::ValidationError { message: "at `/a`: expected integer, got string" })

// Dispatcher that loads every fixture in tests/fixtures/mcp-spec/typed/:
let dispatcher = TypedDispatcher::from_fixtures();

let mut calls = 0;
let result = dispatcher.dispatch(
    "add",
    &serde_json::json!({"a": 2, "b": 3}),
    |args| {
        calls += 1;
        // handler receives the JSON Value; deserialize as you like
        Ok(serde_json::json!({
            "a": args["a"].as_i64().unwrap(),
            "b": args["b"].as_i64().unwrap(),
        }))
    },
);
assert!(result.is_ok());
assert_eq!(calls, 1);
```

When validation fails the handler is **not** invoked (a handler
counter stays at zero); the caller receives a
`ToolError::ValidationError` with a JSON Pointer to the offending
field.

### No `rmcp` dependency

The hard rule from `todo.json v2.0` is reaffirmed here: the typed
layer does not add `rmcp` or any MCP SDK to `Cargo.toml`. The
validator is implemented on top of `serde_json::Value` and the
JSON-Schema subset that the project's fixtures actually use (the
six rows in the threat-model table above). Adding a JSON-Schema
dependency would violate the project's "no second MCP SDK"
principle and is deliberately avoided.

```bash
# Verify the hard rule:
grep -i rmcp tokitai-mcp-server/Cargo.toml
# (no output)
```

### Test surface

`tokitai-mcp-server/tests/mcp_typed_layer_test.rs` covers:

- 5 positive cases (one per fixture tool).
- 7 negative cases (wrong type, missing field, extra property,
  out-of-range, overlong string, non-object root, unknown tool).
- 4 fuzz cases (100 random input shapes per fixture tool;
  malformed inputs are refused; handler invocation count matches
  the number of valid inputs).
- 3 validator-only checks (array element pointer, boolean/null,
  nested-object pointer).

Tests run identically with `--features mcp-typed` and
`--no-default-features`; the feature gate is verified separately
in CI by `cargo build --no-default-features` and
`cargo build --features mcp-typed`.

## Capability model (T-023)

**Threat model**: Tencent Cloud's 2026-06-19 AI security report
identifies the "super-user problem" as the root cause of injection
severity. The standard mitigation (split the agent into N
specialized sub-agents, each with a narrow permission) is
hand-rolled and never enforced. T-023 makes it structural: every
`#[tool]` method declares the capabilities it requires, the
`#[tool]` macro emits a `CAPABILITIES_*` manifest, and the MCP
server refuses to start when the operator-supplied allowlist does
not cover the declared capabilities.

### Declaration shape

```rust
use tokitai::tool;

#[tool]
impl EmailTools {
    /// Send an email to a customer.
    #[tool(
        desc = "Send an email to a customer. Subject and body are required string parameters.",
        requires = ["db:read:sales", "net:egress:smtp"]
    )]
    pub fn send_email(&self, subject: String, body: String) -> String {
        // ...
    }
}
```

The macro emits one `pub const CAPABILITIES_SEND_EMAIL: &[&str]`
per method plus a per-impl aggregated `pub const CAPABILITIES:
&[(&str, &[&str])]`. The MCP server walks the aggregated slice at
startup.

### Operator allowlist

```rust
use tokitai_mcp_server::{McpServerBuilder, serve_with_manifest};

let allowlist = serve_with_manifest(&["db:read:*", "net:egress:smtp"]);

let server = McpServerBuilder::with_tool(EmailTools::default())
    .with_capability_allowlist(allowlist)
    .with_port(8080)
    .build();

server.run().await?; // Ok — allowlist covers every declared cap
```

The `serve_with_manifest(&[&str])` helper is the documented entry
point for operators who do not want to construct the
`Vec<String>` by hand at every call site. The `with_capability_allowlist(Vec<String>)`
builder method is the equivalent for code that already has a
`Vec<String>` in scope.

### Fail-closed contract

When the operator sets an allowlist (even an empty one), the
fail-closed contract applies: any tool that declares a capability
not covered by the allowlist causes the server to refuse to bind.
The typed error is `ServerError::CapabilityNotInAllowlist { tool,
missing }`, returned from `McpServerWithProvider::run_with_address`
before the listener binds. Operators who do NOT set an allowlist
keep the historical behaviour (no allowlist check fires; existing
deployments continue to work).

The fail-closed rule is the whole point of T-023: an operator
who runs `McpServerBuilder::with_tool(provider).with_capability_allowlist(vec![])`
sees a `ServerError::CapabilityNotInAllowlist` for any tool that
declares *any* capability, forcing them to think about the blast
radius before exposing the server to an LLM.

### Wildcard matching

Allowlist entries may use a trailing `*` for prefix matching:

| Allowlist entry | Matches | Does not match |
|-----------------|---------|----------------|
| `db:read:*`     | `db:read:sales`, `db:read:any_resource` | `db:write:sales`, `net:egress:smtp` |
| `*`             | any capability | (n/a) |
| `db:read:sales` | `db:read:sales` (exact) | `db:read:sales_archive` |

Wildcards are recommended in the **operator** allowlist, not in
the **tool** declaration. The recommended practice is to declare
exact capabilities in code (so the blast radius is visible to
reviewers) and to write wildcard allowlist entries in deployment
config (so the operator can match a category with one line).

### Default categories

The recommended category set, mirrored in the T-023 warn-time
diagnostic:

| Prefix                       | Meaning                                  | Example                          |
|------------------------------|------------------------------------------|----------------------------------|
| `db:read:<resource>`         | Read access to a named resource          | `db:read:sales`, `db:read:users` |
| `db:write:<resource>`        | Insert / update access                   | `db:write:audit_log`             |
| `db:delete:<resource>`       | Destructive access                       | `db:delete:users`                |
| `net:egress:<proto>`         | Outbound network on a given protocol     | `net:egress:smtp`, `net:egress:http` |
| `fs:read:<path>`             | Read a file path                         | `fs:read:/var/log/app.log`       |
| `fs:write:<path>`            | Write / create a file path               | `fs:write:/tmp/email_draft.txt`  |
| `fs:delete:<path>`           | Delete a file path                       | `fs:delete:/tmp/cache.json`      |
| `process:exec`               | Spawn a subprocess                       | `process:exec`                   |
| `mail:send`                  | Send an email (alias for net:egress:smtp)| `mail:send`                      |
| `auth:assume:<role>`         | Assume an IAM / OAuth role               | `auth:assume:read_only`          |

The list is **suggested**, not enforced: the macro accepts any
string in `requires = [...]`, and the allowlist accepts any string
too. Operators are free to extend the namespace (e.g.
`slack:post:<channel>`) as long as both the tool declaration and
the allowlist agree on the token.

### Warn-but-pass for missing `requires`

A method that does NOT declare `requires = [...]` triggers a
`W023` warning at compile time (e.g. `[tokitai] [W023] method
\`foo\` has no \`requires = [...]\` manifest; ...`). The warning
is **warn-only** in this release; the follow-up release flips it
to a hard error. The warning can be silenced per-method with
`#[tool(allow = ["missing_capabilities"])]`.

The default-build (no `TOKITAI_QUIET=1`) is intentionally noisy:
the warning is the only mechanism that surfaces the missing
manifest to the user, and silently opting everyone in would
defeat the T-023 design.  Note that `tokitai-macros/build.rs`,
`tokitai/build.rs`, and `tokitai-mcp-server/build.rs` all set
`TOKITAI_QUIET=1` by default (to keep test and CI output clean),
so W023 is **silent in first-party crate builds** — user crates
that do not set `TOKITAI_QUIET` see the warning.

### No new dependencies

The capability manifest type is `pub type CapabilityManifest = Vec<(String, Vec<String>)>` in
`tokitai-core`. The allowlist is `Vec<String>`. The matcher
(`capability_in_allowlist`) lives in `tokitai-core` and uses only
`std`. No new crates are pulled in. The `#[tool]` macro
auto-implements `CapabilityManifestProvider` for every `impl` block
it processes; the trait's default implementation returns an empty
slice, so unit structs and empty impl blocks keep their current
behaviour.

### Test surface

- `tokitai-macros/tests/capabilities_macro_test.rs` (3 tests):
  per-method consts reachable, aggregated manifest correct,
  trait method matches the aggregated const.
- `tokitai-macros/tests/ui/capabilities_requires_basic.rs` +
  `tests/ui/capabilities_requires_non_string.rs` (trybuild):
  positive shape compiles, non-string entry is a compile error.
- `tokitai-mcp-server/tests/capability_manifest_test.rs` (7
  tests): positive, negative, wildcard, warn-but-pass, builder
  ergonomics, typed error shape, and multi-provider aggregation.
- `tokitai-core` unit tests in
  `mod capability_manifest_tests`: exact match, wildcard prefix,
  empty allowlist (fail-closed), bare `*`.
