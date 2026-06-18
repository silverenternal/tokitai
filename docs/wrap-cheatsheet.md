# Tokitai Wrap Features — Cheatsheet

**Version**: 0.5.0 | One-page quick reference for `#[tool]`,
`#[wrap]`, `#[openapi]` / `#[openapi_op]`, `#[delegate]`, and the
three resilience decorators.

---

## All attributes at a glance

| Attribute                                | Where it goes    | What it does                                                                 | Generated extras                                  |
|------------------------------------------|------------------|------------------------------------------------------------------------------|---------------------------------------------------|
| `#[tool]`                                | `impl` block     | Expose every `pub` method as an AI tool                                      | `call_tool`, `__TOOL_DEF_*`, `__call_*`           |
| `#[wrap(client = T, methods = [...])]`   | `impl` block     | Curate a subset of methods of a client struct                                | `pub fn new(client: T) -> Self`                   |
| `#[openapi(spec = "...", base_url = "...")]` | `impl` block | Expose every operation in an OpenAPI 3 spec                                   | `phf::Map<operationId, Op>`, `__OPENAPI_SPEC_RAW` |
| `#[openapi_op(operation_id = "...")]`    | method           | Bind a method to a specific `operationId` in the spec                        | (used by `#[openapi]`)                            |
| `#[delegate(to = "self.inner")]`         | method signature | Forward to an inner method without writing a body                            | `__TOOL_DEF_*`, `__call_*` (no dispatcher)        |
| `#[retry(max = 3, backoff = "exponential", jitter = true)]` | method           | Retry on `Err` with backoff                                                  | rewrites body                                     |
| `#[rate_limit(rps = 10, burst = 20)]`    | method           | Token-bucket throttle                                                        | rewrites body (atomic CAS)                        |
| `#[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]` | method           | 3-state breaker: closed / open / half-open                                   | rewrites body (3 atomics)                         |

---

## Three-line examples

### `#[tool]`

```rust
#[tool]
impl Calculator {
    /// Add two numbers
    pub fn add(&self, a: i32, b: i32) -> i32 { a + b }
}
```

### `#[wrap]`

```rust
#[wrap(client = reqwest::Client, methods = [get_user, list_repos])]
impl GitHubClient { /* pub fn get_user(&self, …) { self.client.get(…) } */ }
```

### `#[openapi]` + `#[openapi_op]`

```rust
#[openapi(spec = "openai_chat.json", base_url = "https://api.openai.com/v1")]
impl OpenAIClient {
    #[openapi_op(operation_id = "createChatCompletion")]
    pub async fn create_chat_completion(&self, body: ChatRequest) -> Result<ChatResponse, reqwest::Error> { /* … */ }
}
```

### `#[delegate]`

```rust
impl OpenAIClient {
    #[delegate(to = "self.inner")]
    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, OpenAIError>;
}
```

### `#[retry]`

```rust
#[retry(max = 3, backoff = "exponential", jitter = true)]
async fn fetch(&self, url: String) -> Result<String, String> { /* … */ }
```

### `#[rate_limit]`

```rust
#[rate_limit(rps = 10, burst = 20)]
fn log_event(&self, message: String) -> String { /* … */ }
```

### `#[circuit_breaker]`

```rust
#[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]
async fn call_external(&self, endpoint: String) -> Result<String, String> { /* … */ }
```

### Multi-schema export

```rust
let td = MyClient::find_tool("get_weather").unwrap();
let openai    = td.to_openai_function();  // { type: "function", function: { … } }
let anthropic = td.to_anthropic_tool();   // { name, description, input_schema }
let mcp       = td.to_mcp_tool();         // { name, description, inputSchema }
```

---

## Common combinations

### 1. Wrap a third-party client, then add resilience

```rust
#[wrap(client = reqwest::Client, methods = [get_user, list_repos])]
impl GitHubClient {
    #[retry(max = 3)]
    #[rate_limit(rps = 5, burst = 10)]
    pub async fn get_user(&self, login: String) -> Result<User, String> { /* … */ }
}
```

### 2. Stack resilience decorators — outer wins

```rust
#[retry(max = 3)]                 // ② re-runs the call on Err
#[rate_limit(rps = 10)]          // ① throttles the call
async fn call(&self) -> Result<_, _> { /* body */ }
```

### 3. `#[delegate]` inside a `#[wrap]`

```rust
#[wrap(client = OpenAISdk, methods = [chat, embed])]
impl MyClient {
    #[delegate(to = "self.client")]
    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, OpenAIError>;

    #[delegate(to = "self.client")]
    pub async fn embed(&self, input: String) -> Result<Vec<f32>, OpenAIError>;
}
```

### 4. Use `#[tool]` for your own code, `#[openapi]` for an external API

```rust
#[tool]
impl MyService { /* your domain methods */ }

#[openapi(spec = "openai_chat.json", base_url = "https://api.openai.com/v1")]
impl OpenAIClient { /* vendored endpoints */ }
```

---

## Quick reminders

- **All wrap features are compile-time.** The only runtime cost is the
  generated code itself.
- **Resilience decorators wrap the body, not the dispatcher.** They
  see a `Result<T, E>` function and emit a new body.
- **Multi-schema export is just three methods on `ToolDefinition`.**
  The same `input_schema: String` is wrapped in OpenAI / Anthropic /
  MCP envelopes.
- **For async + `#[retry]` / `#[rate_limit]` / `#[circuit_breaker]`,
  the inter-attempt sleep is driven through `tokitai_core::async_sleep(...)`
  (T-004), which yields to whichever executor is in scope (Tokio,
  async-std, smol, ...). The sleep does **not** call
  `std::thread::sleep` on the async path; the runtime worker is
  never blocked. No `AsyncExecutor` needs to be registered for the
  decorator to compile or run, but registering one is the way to
  hand the wait off to a real timer (rather than spawning a fresh
  thread per wait).

See [`wrap-architecture.md`](wrap-architecture.md) for the full
deep-dive, composition rules, and limitations.
