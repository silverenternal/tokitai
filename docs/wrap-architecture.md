# Tokitai Wrap Features: Architecture

**Version**: 0.5.0 | **Audience**: Rust developers evaluating Tokitai's
auto-wrapping proc-macro attributes

Tokitai's **`#[tool]`** proc-macro turns the public methods of an `impl`
block into AI-callable tools. The five **wrap features** — `#[wrap]`,
`#[openapi]` / `#[openapi_op]`, `#[delegate]`, and the three resilience
decorators `#[retry]` / `#[rate_limit]` / `#[circuit_breaker]` — extend
that pipeline to cover the common case of "I already have a client / API
/ trait I want to expose; please do the boring bits for me." This
document describes them as a single, unified system.

---

## Table of contents

1. [Introduction](#1-introduction)
2. [Feature matrix](#2-feature-matrix)
3. [Architecture overview](#3-architecture-overview)
4. [Per-feature deep-dive](#4-per-feature-deep-dive)
5. [Composition rules](#5-composition-rules)
6. [Performance characteristics](#6-performance-characteristics)
7. [Limitations and future work](#7-limitations-and-future-work)
8. [Dialect correctness](#8-dialect-correctness)

---

## 1. Introduction

### The problem

Most Rust projects that want to expose functionality to an LLM already
have a hand-written (or `#[derive]`-generated) client. Examples:

- A `reqwest::Client`-backed struct with one method per REST endpoint.
- A wrapper around an OpenAPI-generated client (often the output of
  `progenitor`, `openapi-generator`, or `oapi-codegen`).
- A typed handle to a database driver, message queue, or third-party
  SDK.

Writing `#[tool]` over every method by hand is tedious and brittle: every
new method must be remembered, the dispatcher must be kept in sync, and
none of the cross-cutting concerns (retries, rate limits, circuit
breakers) are visible to the macro.

### The solution

The five **wrap features** make `#[tool]` aware of these patterns:

| Feature                | What it auto-generates for you                                     |
|------------------------|--------------------------------------------------------------------|
| `#[wrap]`              | A `new(client)` constructor + a curated list of tool methods      |
| `#[openapi]` + `#[openapi_op]` | A `phf::Map` lookup of every operation in a spec file          |
| `#[delegate]`          | A forwarded method body, so you write signatures, not bodies      |
| `#[retry]`             | A retry loop with configurable backoff                             |
| `#[rate_limit]`        | A token-bucket guard, lock-free (atomic CAS)                      |
| `#[circuit_breaker]`   | A 3-state (closed / open / half-open) machine, lock-free          |

All six are **proc-macro attributes**, all six emit code at **compile
time**, and all six share the same downstream `__TOOL_DEF_*` / `__call_*`
artifacts that `#[tool]` already produces. The runtime cost of a
wrapped tool is identical to the runtime cost of an unwrapped one,
plus the small per-call overhead of whatever resilience decorators
you stacked on top.

A single `ToolDefinition` (the type emitted by every macro) can also be
serialised into three different tool descriptor envelopes —
**OpenAI**, **Anthropic**, and **MCP** — via three methods on
`ToolDefinition` itself. See
[multi-schema export](#multi-schema-export).

---

## 2. Feature matrix

| Feature                  | Use case                                        | Output                                                        | Async-aware? | Runtime cost (per call)                                 | Doc / example                                                                |
|--------------------------|-------------------------------------------------|---------------------------------------------------------------|--------------|---------------------------------------------------------|------------------------------------------------------------------------------|
| `#[tool]` (baseline)     | Your own `impl` block; expose all `pub` methods | `call_tool` dispatcher, `ToolProvider`/`ToolCaller` impls     | Yes          | Argument parsing + a `match`                            | [`README.md`](../README.md) / `examples/basic_usage.rs`                      |
| `#[wrap]`                | Curate which methods of a client struct to expose | `new(client)` + the same artifacts as `#[tool]`              | Yes          | Same as `#[tool]`                                       | **tracking-issue:** [#31](https://github.com/silverenternal/tokitai/issues/31); pattern demo: `examples/wrap_demo.rs` |
| `#[openapi]` / `#[openapi_op]` | Wrap a whole REST API from an OpenAPI 3 spec | `phf::Map<operationId, Op>` + the same per-method artifacts    | Yes          | Same as `#[tool]`; no spec parsing at runtime          | **tracking-issue:** [#36](https://github.com/silverenternal/tokitai/issues/36) |
| `#[delegate]`            | Re-expose an inner method with no body          | Forwarded body + `__TOOL_DEF_*` + `__call_*` (no dispatcher)  | Yes          | Zero (the body is a single field access)                | **tracking-issue:** [#32](https://github.com/silverenternal/tokitai/issues/32) |
| `#[retry]`               | Make a flaky call self-healing                  | A `loop { … }` around the body with backoff                   | Yes          | One `Result` match + (optional) sleep per attempt       | **tracking-issue:** [#33](https://github.com/silverenternal/tokitai/issues/33) |
| `#[rate_limit]`          | Throttle an external API                        | A token-bucket pre-guard                                      | Yes          | One atomic CAS per call (lock-free)                     | **tracking-issue:** [#34](https://github.com/silverenternal/tokitai/issues/34) |
| `#[circuit_breaker]`     | Stop hammering a known-broken dependency        | A 3-state machine, updated pre/post                           | Yes          | Three `AtomicU8/U32/U64` loads/stores per call          | **tracking-issue:** [#35](https://github.com/silverenternal/tokitai/issues/35) |
| Multi-schema export      | Feed the same `ToolDefinition` to OpenAI / Anthropic / MCP | Three `serde_json::Value` envelopes              | n/a          | Three `serde_json::json!` calls (cheap)                 | `tokitai-core/src/lib.rs` (`to_openai_function` etc.)                         |

> **Note (0.5.x):** The `#[wrap]`, `#[delegate]`, `#[retry]`, `#[rate_limit]`, `#[circuit_breaker]`, and `#[openapi]` proc-macro attributes are implemented in `tokitai-macros/src/tool/{wrap,delegate,resilience,wrap_openapi}/` but are **not yet exposed** through the public `tokitai` / `tokitai_macros` re-exports. Until each tracking issue above is closed, the design pattern they target is exercised end-to-end by `examples/wrap_demo.rs`, which uses the stable `#[tool]` + `MultiToolProvider` surface.

---

## 3. Architecture overview

```text
 ┌──────────────────────────────────────────────────────────────────────────┐
 │                      USER SOURCE CODE (Cargo project)                    │
 │                                                                          │
 │   #[wrap(client = InnerClient, methods = [get_user, list_repos])]        │
 │   impl GitHubClient { pub fn get_user(&self, login: String) -> … { … } }  │
 │                                                                          │
 │   #[openapi(spec = "openai.json", base_url = "https://api.openai.com")]  │
 │   impl OpenAIClient { #[openapi_op(operation_id="…")] async fn … {…} }   │
 │                                                                          │
 │   #[retry(max=3)] #[rate_limit(rps=10)] async fn fetch(…) -> … { … }     │
 └────────────────────────────────┬─────────────────────────────────────────┘
                                  │   proc-macro compile time
                                  ▼
 ┌──────────────────────────────────────────────────────────────────────────┐
 │   tokitai-macros (proc-macro crate)                                     │
 │                                                                          │
 │   1.  Parse the attribute (#[wrap], #[openapi], #[delegate], …)          │
 │   2.  Parse the impl block / method signature                            │
 │   3.  For #[openapi]: `std::fs::read_to_string` + `serde_json` parse +   │
 │       `phf::Map` bake                                                    │
 │   4.  For resilience: rewrite the function body in place                 │
 │   5.  Emit:                                                              │
 │        - `pub fn __TOOL_DEF_<NAME>() -> ToolDefinition`                  │
 │        - `fn __call_<NAME>(args: &Value) -> Result<Value, ToolError>`    │
 │        - `fn call_tool(name, args) -> …` (dispatcher)                    │
 │        - `impl ToolProvider for …` + `impl ToolCaller for …`             │
 │        - `static __TOOL_COUNT: usize`                                    │
 └────────────────────────────────┬─────────────────────────────────────────┘
                                  │   emitted Rust source
                                  ▼
 ┌──────────────────────────────────────────────────────────────────────────┐
 │   tokitai-core (no_std-friendly types)                                  │
 │                                                                          │
 │   struct ToolDefinition { name, description, input_schema: String }     │
 │                                                                          │
 │   trait ToolProvider {                                                   │
 │       fn tool_definitions() -> &'static [ToolDefinition];                │
 │       fn tool_count() -> usize;                                          │
 │       fn find_tool(name) -> Option<&'static ToolDefinition>;             │
 │   }                                                                      │
 │                                                                          │
 │   trait ToolCaller {                                                     │
 │       fn call_tool(&self, name, args) -> Result<Value, ToolError>;       │
 │   }                                                                      │
 │                                                                          │
 │   impl ToolDefinition {                                                  │
 │       fn to_openai_function(&self)  -> serde_json::Value  // envelope #1  │
 │       fn to_anthropic_tool(&self)   -> serde_json::Value  // envelope #2  │
 │       fn to_mcp_tool(&self)         -> serde_json::Value  // envelope #3  │
 │   }                                                                      │
 └────────────────────────────────┬─────────────────────────────────────────┘
                                  │   runtime
                                  ▼
 ┌──────────────────────────────────────────────────────────────────────────┐
 │   Downstream consumers                                                   │
 │                                                                          │
 │   - tokitaiexamples/basic_usage.rs   : "do math"                         │
 │   - tokitaiexamples/mcp_http_server.rs : serves the same tools over HTTP │
 │   - tokitaiexamples/curl/  python/  go/  js/  : client SDKs              │
 │   - tokitai-mcp-server     : full MCP server (`/tools`, `/call`)         │
 │                                                                          │
 │   Each consumer picks the envelope it needs:                             │
 │       for tool in MyClient::tool_definitions() {                         │
 │           let v = tool.to_openai_function(); // or anthropic / mcp       │
 │           send_to_llm(v);                                                │
 │       }                                                                  │
 └──────────────────────────────────────────────────────────────────────────┘
```

### Key invariants

1. **All tool definitions are compile-time constants.** A `ToolDefinition`
   holds `&'static str` (or an owned `String` filled from a `&'static str`
   via `from_const`). No `lazy_static` / `OnceCell` is needed for the
   schema itself.
2. **All schema parsing happens at proc-macro time.** The OpenAPI
   spec is read off disk, parsed with `serde_json`, and the result is
   `phf`-baked into the binary. The user's program does no spec
   parsing at runtime.
3. **Resilience decorators wrap the body, not the dispatcher.** They
   see a function whose return type is `Result<T, E>` and emit a new
   body that re-runs the original block on `Err`. The `#[tool]`
   dispatcher and `__call_*` wrappers sit outside this transformation.
4. **A `ToolDefinition` is the lingua franca.** It is the only type a
   consumer needs to know about. The three envelope methods turn it
   into whatever shape the LLM provider expects.

---

## 4. Per-feature deep-dive

### 4.1 `#[wrap]`

**Signature-driven**, designed for wrapping a local client struct.
Minimal macro overhead — it reuses 100% of the `#[tool]` codegen
pipeline; the only thing it adds is a `new(client: T) -> Self`
constructor.

#### When to reach for it

You have a struct (or could trivially make one) that owns a
third-party client, and you want a curated list of methods to show
up as tools.

#### Example

```rust
use tokitai::wrap;
use tokitai::ToolProvider;

pub struct GitHubClient {
    pub client: reqwest::Client,
}

#[wrap(client = reqwest::Client, methods = [get_user, list_repos])]
impl GitHubClient {
    /// Look up a GitHub user by login.
    pub async fn get_user(&self, login: String) -> Result<User, String> {
        let r = self.client
            .get(format!("https://api.github.com/users/{login}"))
            .send().await.map_err(|e| e.to_string())?;
        Ok(r.json().await.map_err(|e| e.to_string())?)
    }

    /// List public repositories owned by `owner`.
    pub async fn list_repos(&self, owner: String) -> Result<Vec<Repo>, String> {
        /* ... */
    }
}
```

#### Generated artifacts

Identical to `#[tool]` (`__TOOL_DEF_GET_USER`, `__call_get_user`,
`call_tool`, `impl ToolProvider`, `impl ToolCaller`, `__TOOL_COUNT`)
**plus** a generated `pub fn new(client: reqwest::Client) -> Self`
that wires the inner client into the `client` field. Use
`field = "my_field"` to override the field name.

---

### 4.2 `#[openapi]` / `#[openapi_op]`

**Spec-driven**. Parses an OpenAPI 3 file at compile time. The spec is
`include_str!`-ed into the consumer crate, parsed with `serde_json`,
and stored in a `phf::Map<&str, __OpenApiOp_*>` keyed by
`operationId`. The five-line setup below exposes an entire REST API
as tools.

#### When to reach for it

You have an OpenAPI spec (yours or a vendor's), you don't want to
hand-write a Rust client, and you're happy to write the `reqwest`
calls yourself while letting the macro handle the metadata.

#### Example

```rust
use tokitai::{openapi, openapi_op, ToolProvider};

#[openapi(spec = "openai_chat.json", base_url = "https://api.openai.com/v1")]
impl OpenAIClient {
    #[openapi_op(operation_id = "createChatCompletion")]
    pub async fn create_chat_completion(
        &self,
        body: ChatRequest,
    ) -> Result<ChatResponse, reqwest::Error> {
        self.http
            .post(format!("{}/chat/completions", self.base_url()))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await?
            .json().await
    }
}

let defs = OpenAIClient::tool_definitions();
assert!(!defs.is_empty());
```

#### Generated artifacts

For every `#[openapi_op]`-marked method: the usual `__TOOL_DEF_*`,
`__call_*`, and `call_tool`. In addition:

- `pub static __OPENAPI_OPS_<Type>: phf::Map<&str, __OpenApiOp_<Type>>`
  — keyed by `operationId`; useful for runtime introspection.
- `pub static __OPENAPI_SPEC_RAW: &str` — the raw spec text, useful
  for `$ref` resolution or pretty-printing.

#### Constraints

- The spec must be **OpenAPI 3.0** JSON. YAML is not supported
  (convert with `yq`). OpenAPI 3.1 is partially supported; Swagger
  2.0 is not.
- The file path is resolved at proc-macro compile time — relative
  paths are interpreted relative to the file containing the
  `#[openapi]` invocation.
- Operations missing an `operationId` are still indexed under
  `"<METHOD> <path>"` in `__OPENAPI_OPS_*`, but cannot be bound to a
  method (the `#[openapi_op]` attribute requires `operation_id`).

---

### 4.3 `#[delegate(to = "...")]`

**Method-level transparent forwarding**, zero body needed. You write
the signature, the macro injects a body that evaluates
`<to>.<method_name>(<args>)` (or `.await` for `async fn`).

#### When to reach for it

You have a `reqwest::Client` / SDK / database driver that already
implements the methods you want to expose, and you don't want to
write `self.inner.foo(x)` for every single one.

#### Example

```rust
use tokitai::delegate;

pub struct OpenAIClient {
    pub inner: OpenAISdk,
}

impl OpenAIClient {
    #[delegate(to = "self.inner")]
    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, OpenAIError>;

    #[delegate(to = "Config::default()")]
    pub fn default_config() -> Config;
}
```

#### Generated artifacts

1. The forwarded method, with the same signature, and a generated
   body that evaluates `<to>.<method_name>(<args>)`.
2. A `__TOOL_DEF_<NAME>()` function.
3. A `__call_<NAME>(args)` wrapper, plus a `__call_<NAME>_sync`
   variant for `async fn`.

`#[delegate]` deliberately does **not** emit a `call_tool` dispatcher
or a `ToolProvider` impl. It is meant to be combined with `#[tool]`
or used standalone with a manual dispatcher — emitting a dispatcher
unconditionally would clash with `#[tool]`'s own dispatcher.

---

### 4.4 `#[retry]`

**Pre/post guard**: wraps the body of a function in a retry loop. The
function must return `Result<T, E>`; on each `Err` the macro sleeps
for a backoff interval and re-runs the body, up to a configurable
maximum.

```rust
use tokitai_macros::retry;

#[retry(max = 3, backoff = "exponential", jitter = true)]
async fn fetch(&self, url: String) -> Result<String, String> {
    // body that may return Err
}
```

#### Args

| Arg        | Type   | Default        | Meaning                                                                                  |
|------------|--------|----------------|------------------------------------------------------------------------------------------|
| `max`      | `u32`  | `3`            | Maximum number of attempts                                                               |
| `backoff`  | `str`  | `"exponential"`| One of `"constant"`, `"linear"`, `"exponential"`                                         |
| `jitter`   | `bool` | `true`         | Add a small random offset derived from `SystemTime::now().subsec_nanos()`                |
| `on`       | `str`  | `"any"`        | Accepted in v1 for forward compatibility; v1 always retries on any `Err`                 |

Works on both **sync** and **async** functions. For async functions
the sleep is driven by `tokitai_core::async_sleep(...)` (T-004),
which yields to whatever executor is in scope (Tokio, async-std,
smol, ...). It does **not** block the runtime worker thread and
does not fall back to `std::thread::sleep` on the async path. For
sync functions the sleep is `std::thread::sleep` (acceptable
because the caller is already a sync caller).

---

### 4.5 `#[rate_limit]`

**Pre-guard** with token-bucket semantics. The first `burst` calls
proceed immediately; subsequent calls are throttled to `rps` per
second.

```rust
use tokitai_macros::rate_limit;

#[rate_limit(rps = 10, burst = 20)]
fn log_event(&self, message: String) -> String { /* ... */ }
```

#### Args

| Arg     | Type  | Default | Meaning                       |
|---------|-------|---------|-------------------------------|
| `rps`   | `u32` | `1`     | Sustained requests per second |
| `burst` | `u32` | `1`     | Maximum burst size            |

The implementation is **lock-free**: one `AtomicU32` (current tokens)
and one `AtomicU64` (last refill timestamp) per decorated function,
with a single 32-bit CAS per call. Works on both sync and async.

---

### 4.6 `#[circuit_breaker]`

**Pre/post guard** with a 3-state machine (closed / open / half-open).
After `failure_threshold` consecutive `Err`s the breaker opens; after
`reset_timeout` it transitions to half-open and probes with the next
call.

```rust
use tokitai_macros::circuit_breaker;

#[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]
async fn call_external(&self, endpoint: String) -> Result<String, String> {
    /* ... */
}
```

#### Args

| Arg                 | Type   | Default | Meaning                                                                |
|---------------------|--------|---------|------------------------------------------------------------------------|
| `failure_threshold` | `u32`  | `5`     | Consecutive `Err`s before the breaker opens                            |
| `reset_timeout`     | `str`  | `"30s"` | `"30s"`, `"500ms"`, `"2m"`, `"1h"`, or a bare integer (seconds)         |

State is held in three static atomics per decorated function
(`AtomicU8` for state, `AtomicU32` for failure counter, `AtomicU64`
for `open_at_ns` timestamp).

#### v1 limitation: fail-fast

v1 does **not** synthesise an error when the breaker is open;
instead it lets the body run so the call still observes the current
state. This keeps the error type generic (no `E: From<String>` bound
on the user's error). v2 will introduce a `CircuitOpen` trait the
user's error type implements, enabling true fail-fast.

---

### 4.7 Multi-schema export

Three methods on `ToolDefinition` wrap the same `input_schema: String`
in three different envelopes. They are the bridge from a Tokitai
`ToolDefinition` to whatever shape the LLM provider expects.

```rust
let td: &ToolDefinition = MyClient::find_tool("get_weather").unwrap();
let openai    = td.to_openai_function();    // { type: "function", function: { … parameters } }
let anthropic = td.to_anthropic_tool();     // { name, description, input_schema }
let mcp       = td.to_mcp_tool();           // { name, description, inputSchema }
```

| Method                | Field name       | Top-level shape                                        |
|-----------------------|------------------|--------------------------------------------------------|
| `to_openai_function`  | `parameters`     | `{ "type": "function", "function": { … } }`            |
| `to_anthropic_tool`   | `input_schema`   | `{ name, description, input_schema }`                  |
| `to_mcp_tool`         | `inputSchema`    | `{ name, description, inputSchema }`                   |

If the stored `input_schema` is not valid JSON, each method emits an
empty object (`{}`) in its place so the surrounding envelope is
always well-formed — this matters because partial schema failures
should not break the whole `tool_definitions()` call.

See `tokitai-core/tests/multi_schema_export_test.rs` for the
field-by-field assertions.

---

## 5. Composition rules

The general rule: **outer attribute wraps inner attribute**. Rust
applies proc-macro attributes in source order, outermost-last, so
the topmost attribute sees the already-rewritten body of the
attribute beneath it.

### Combinations that work

| Combination                                          | Effect                                                                 |
|------------------------------------------------------|------------------------------------------------------------------------|
| `#[tool]` + `#[retry]` on a method                   | Retry wraps the body; the `#[tool]` dispatcher then sees the outer fn |
| `#[wrap]` + `#[delegate]` on methods in the same impl | `#[wrap]` lists which `#[delegate]`-marked methods become tools     |
| `#[openapi]` + `#[retry]` per method                 | `#[openapi_op]` binds the method; `#[retry]` adds resilience           |
| `#[retry]` + `#[rate_limit]`                         | `#[rate_limit]` is applied first (innermost); `#[retry]` wraps the rate-limited call in a retry loop |
| `#[rate_limit]` + `#[circuit_breaker]`               | `#[rate_limit]` first; `#[circuit_breaker]` second (rate-limits even the probe) |

### Reading order

```rust
#[retry(max = 3)]              // ③ outermost: re-runs the whole call on Err
#[rate_limit(rps = 10)]       // ② middle: throttles before the body
#[circuit_breaker(            // ① innermost: pre-checks state, post-records result
    failure_threshold = 5,
    reset_timeout = "30s",
)]
async fn call(&self) -> Result<_, _> { /* body */ }
```

This is equivalent to:

```text
retry {
    rate_limit {
        circuit_breaker {
            // original body
        }
    }
}
```

### A worked example

```rust
#[tool]
impl WeatherClient {
    /// Fetch the weather for a city, with retries and a soft cap on QPS.
    #[retry(max = 3, backoff = "exponential")]
    #[rate_limit(rps = 5, burst = 10)]
    pub async fn get_weather(&self, city: String) -> Result<Weather, Error> {
        self.http
            .get(format!("https://api.weather.example/{city}"))
            .send().await?
            .json().await
    }
}
```

The generated `__call_get_weather` invokes this wrapped method, so
every call through the dispatcher is automatically throttled and
auto-retried.

---

## 6. Performance characteristics

All five wrap features are **compile-time**. The only runtime cost
is the generated code itself.

| Feature                  | Static state allocated                   | Per-call work (hot path)                                                  |
|--------------------------|------------------------------------------|---------------------------------------------------------------------------|
| `#[wrap]`                | none                                     | Same as `#[tool]`                                                         |
| `#[openapi]`             | one `phf::Map` per impl, `&'static str`  | One `phf::Map` lookup + same as `#[tool]`                                 |
| `#[delegate]`            | none                                     | One field-access forward                                                 |
| `#[retry]`               | none                                     | A `Result` match + an awaited `tokitai_core::async_sleep(...)` between attempts (T-004) |
| `#[rate_limit]`          | one `AtomicU32` + one `AtomicU64`        | One `compare_exchange` (lock-free); occasionally an awaited `tokitai_core::async_sleep(...)` (T-004) |
| `#[circuit_breaker]`     | one `AtomicU8` + one `AtomicU32` + one `AtomicU64` | Three atomic loads in the pre-guard, three stores in the post-handler |
| `to_openai_function` etc.| none                                     | One `serde_json::from_str` + one `json!` (sub-microsecond)                |

In other words, the **fast path** for a wrapped tool is:

1. A `match` on the tool name (one branch).
2. A `FromJsonValue::from_json_value` per parameter (one field access
   + one type check).
3. The body, possibly wrapped in a token-bucket CAS, a retry loop, a
   circuit-breaker check, or all three.

There is no per-call `HashMap` lookup, no spec parsing, no schema
deserialisation, no reflection. The whole pipeline is designed so
that a 1μs `call_tool` benchmark from `#[tool]` (see README) is
unchanged by `#[wrap]` and grows by the cost of the atomics listed
above when a resilience decorator is stacked on top.

---

## 7. Limitations and future work

### 7.1 `#[wrap]`

- **No auto-discovery.** You list methods explicitly in
  `methods = [name1, name2, ...]`. This is the design: `#[tool]`
  is the "every public method" workflow; `#[wrap]` is the "I have
  an API client, expose just these endpoints" workflow.
- The inner client type must be a `syn::Type` (so it accepts
  `std::sync::Arc<T>` and qualified paths).

### 7.2 `#[openapi]`

- **OpenAPI 3.0 only** (JSON; YAML must be converted first). OpenAPI
  3.1 support is partial (some `nullable` / `type: ["…", "null"]`
  constructs are accepted but not all). Swagger 2.0 is **not**
  supported.
- The spec file is read at proc-macro compile time, so the macro
  host needs read access to the spec.
- The macro does not synthesise method bodies — you still have to
  write the `reqwest` calls yourself.

### 7.3 `#[delegate]`

- Requires the `to` expression to be valid in the method's context
  (i.e. `self.inner.foo` must type-check on its own).
- Does not emit a `call_tool` dispatcher. Combine with `#[tool]` or
  wire the dispatcher by hand.

### 7.4 Resilience decorators

- On **`async fn`**, the inter-attempt sleep is driven through
  `tokitai_core::async_sleep(...)`, a runtime-agnostic sleep
  future that yields to whatever executor is in scope (Tokio,
  async-std, smol, ...). The sleep does **not** call
  `std::thread::sleep` on the async path; it spawns a single
  thread per wait that wakes the future's `Waker` when the
  deadline elapses. The runtime worker is never blocked.
  Sync functions fall back to `std::thread::sleep` (acceptable
  because the caller is already a sync caller).
- `#[retry]` and `#[rate_limit]` and `#[circuit_breaker]` are
  per-function; nested `#[retry]` layers in v1 do not stack
  cleanly (the inner layer wins). v2 will detect existing retry
  state and append a new layer.
- `#[circuit_breaker]` v1 does not fail-fast; it lets the body run
  so the user can observe the breaker state. v2 will introduce a
  `CircuitOpen` trait.
- All three carry no extra runtime dependencies (no `tokio`,
  no `governor`); the implementation is built on `std::sync::atomic`
  and `std::time::SystemTime` only. The async sleep uses
  `std::thread::park_timeout` internally so the resilience decorators
  remain executor-agnostic.

### 7.5 Open future work

- Auto-discovery for `#[wrap]` (whitelist / blacklist / `#[tool(skip)]`).
- `#[delegate]` integration with `#[tool]` (one-stop dispatcher).
- OpenAPI 3.1 and Swagger 2.0 parsers.
- v2 composition for resilience decorators (nested `#[retry]`,
  `CircuitOpen` fail-fast, `governor`-style rate-limiter).
- Cross-language clients in `examples/curl/`, `examples/py/`,
  `examples/go/`, `examples/js/` already consume the same
  `ToolDefinition` via the three envelope methods; future work
  is to make those clients first-class and re-export them from
  the top-level crate.

---

## 8. Dialect correctness

**Audience**: anyone shipping a tool whose schema will be
consumed by a known LLM provider (Claude Desktop, Cursor,
the OpenAI Agents SDK, VS Code Copilot, etc.). **TL;DR**:
write `#[tool(dialect = "...")]` on your `impl` block and the
macro will refuse to ship a schema that the chosen provider
will reject — at compile time, not in production.

### 8.1 The pain

Every LLM tool-calling provider ships a slightly different
JSON-Schema dialect. `mcp-lint` exists because Claude
Desktop, Cursor, OpenAI Agents SDK, and VS Code Copilot
disagree on what `required: false`, `additionalProperties:
true`, and `oneOf` siblings mean in MCP-flavored JSON
Schema. Tools that ship a single JSON Schema discover this
at runtime in production — silently, when an LLM tool
call fails in one provider but works in another.

Tokitai's macro knows the Rust types at expansion time and
can refuse to emit a schema that any supported provider will
reject. No runtime-only competitor can do this.

### 8.2 The `dialect = "..."` attribute

Apply the attribute at the impl level:

```rust,ignore
use tokitai::tool;

#[tool(dialect = "openai-strict")]
impl MyTools {
    /// Add two integers.
    pub fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }
}
```

The macro then audits every emitted
`ToolDefinition.input_schema` against the chosen dialect's
rule set. Violations become `compile_error!` invocations
anchored at the user-written method span (T-001) so the
editor jumps straight to the offending code.

| Dialect           | Aliases accepted           | Loose / strict | Reference                                    |
|-------------------|----------------------------|----------------|----------------------------------------------|
| `mcp`             | `mcp`, `MCP`, `Mcp`        | Loosest (default) | MCP 2025-06-18                              |
| `openai-strict`   | `openai-strict`, `openai`, `OpenAI` | Strictest | OpenAI `function.parameters` strict-mode |
| `anthropic`       | `anthropic`, `Anthropic`, `claude`   | Strict | Anthropic `inputSchema`               |

Choosing `mcp` (the default) means the macro does the
loosest check and lets the runtime serialization to
`to_openai_function()` / `to_anthropic_tool()` / `to_mcp_tool()`
do the provider-specific translation. Choosing
`openai-strict` or `anthropic` means the macro audits the
schema against the provider's known quirks **at compile
time** so the bug is caught before the binary ships.

### 8.3 The rule set

The rule set lives in
[`tokitai-macros/src/tool/schema/dialect.rs`](../tokitai-macros/src/tool/schema/dialect.rs).
Each rule is a closure with a stable code:

| Code     | Dialect        | Fires when…                                                                                 |
|----------|----------------|---------------------------------------------------------------------------------------------|
| `MCP-1`  | `mcp`          | A property has no explicit JSON Schema `type` (e.g. raw `serde_json::Value`).              |
| `OA-1`   | `openai-strict`| The root object declares `additionalProperties: true`.                                     |
| `OA-2`   | `openai-strict`| A property has no explicit `type` (including `Option<serde_json::Value>`).                 |
| `OA-3`   | `openai-strict`| A positional tuple shape (`prefixItems`) appears anywhere in the schema.                    |
| `AN-1`   | `anthropic`    | A nested object has no explicit `additionalProperties: false` declaration.                 |

The audit is *post-rendering*: the macro renders the schema
with `serde_json::to_string(...)`, then re-parses the AST
into the `JsonSchema` enum (see
[`gen::generate_schema_ast_and_json_with_deprecated_and_tags`](../tokitai-macros/src/tool/schema/gen.rs))
and walks it recursively. This keeps the rule set simple
(no AST traversal for the variants we have at hand) and
lets the same rule set be used by hand-rolled
`ToolDefinition::new(...)` calls in tests.

### 8.4 Why "kill mcp-lint"?

`mcp-lint` is a separate linter that runs on saved JSON
Schema files and reports dialect violations after the fact.
Tokitai's compile-time audit is the same idea, except it
runs before the binary ships and catches the bug at the
*source* — the Rust signature that would have produced the
non-conformant schema. There's no schema file to lint, no
separate step in CI, and no chance of the developer
forgetting to run it.

The doc above is the single source of truth for the rule
set. If you find a provider quirk that is not yet covered,
open an issue with a failing fixture under
`tokitai-macros/tests/ui/` and we will add a rule.

### 8.5 Defaults and trade-offs

The default dialect is `mcp` (loosest). This is the
**recommendation for 0.6** — see the open design question
`Q-4` in `todo.json` for the trade-off. A user who
silently switches providers gets no warning at compile
time under `mcp`, but the runtime envelope methods
(`to_openai_function()` etc.) translate the schema on
their way out. The stricter dialects are opt-in and
recommended for teams that ship to a known provider and
want the compile-time guarantee.

---

## See also

- [`README.md`](../README.md) — top-level overview
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — overall Tokitai architecture
- [`USAGE.md`](USAGE.md) — `#[tool]` baseline (type mapping, custom
  attributes, parameter constraints)
- [`ADVANCED_USAGE.md`](ADVANCED_USAGE.md) — `tokitai!` config macro,
  custom type schemas, runtime overrides
- [`CROSS_LANGUAGE.md`](CROSS_LANGUAGE.md) — HTTP+JSON protocol and
  SDK quickstarts for Python, JS/TS, Go, curl
- [`API_STABILITY.md`](API_STABILITY.md) — semver policy for the wrap
  features
