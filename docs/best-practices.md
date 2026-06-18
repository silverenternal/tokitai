# Tokitai Best Practices Guide

**Version**: 0.5.x | **Audience**: Rust developers writing AI tools
with Tokitai.

This guide covers the *what* and *why* of macro choice, parameter
design, doc-comment hygiene, error handling, and the patterns that
look like a good idea but are not. For the *how fast* and *how
much memory*, see [`performance.md`](performance.md). For the
*why* of the design, see the [ADRs](adr/README.md).

---

## Table of contents

1. [TL;DR](#tldr)
2. [When to use which macro](#when-to-use-which-macro)
3. [Composing resilience macros](#composing-resilience-macros)
4. [Parameter design](#parameter-design)
5. [Doc comment hygiene](#doc-comment-hygiene)
6. [Error handling](#error-handling)
7. [Testing your tools](#testing-your-tools)
8. [Anti-patterns](#anti-patterns)
9. [See also](#see-also)

---

## TL;DR

Five patterns, expanded below:

1. **Pick the macro by where the methods come from.** `#[tool]`
   for your own `impl`, `#[wrap]` for an existing client,
   `#[openapi]` for a vendored OpenAPI spec, `#[delegate]` for
   one-line forwarding. *See
   [When to use which macro](#when-to-use-which-macro).*
2. **Stack resilience decorators innermost-to-outermost as
   `rate_limit → circuit_breaker → retry`.** The reverse order
   either rate-limits the retry or retries the rate-limit. *See
   [Composing resilience macros](#composing-resilience-macros).*
3. **Make parameters *narrow and named***. `user_id: i32`, not
   `data: serde_json::Value`; `Option<T>` only for genuinely
   optional things. *See [Parameter design](#parameter-design).*
4. **Return `Result<T, E>` (any `Display`-convertible `E`), not
   bare `T`, when failure is possible.** *See
   [Error handling](#error-handling).*
5. **Cache `tool_definitions()` once, test through the
   dispatcher, and never expose internal helpers without
   `#[tool(skip)]`.** *See [Anti-patterns](#anti-patterns).*
---

## When to use which macro

Tokitai has four entry-point proc-macro attributes. All produce
a `ToolProvider` impl, but differ in where the methods come
from and what artifacts they emit. See
[`wrap-architecture.md` §2](wrap-architecture.md#2-feature-matrix)
for the full feature matrix.

### Decision tree

```
Do you already have a Rust client struct that does the work?
├── Yes, expose a curated subset
│       → #[wrap(client = T, methods = [m1, m2, …])]
├── Yes, expose every public method
│       → #[tool] on the impl, with #[tool(skip)] on helpers
├── No, you have an OpenAPI 3 spec
│       → #[openapi(spec = "spec.json")] + #[openapi_op] per method
├── No, single inner method to forward to
│       → #[delegate(to = "self.inner")]
└── No, this is your own domain logic
        → #[tool] on the impl block
```

### `#[tool]` — your own `impl` block

Use when you own the `impl` and want every public method to be
a tool. Avoid when you already have a client struct — `#[tool]`
will expose every `pub` method including helpers you didn't mean
to expose. Use `#[wrap]` or `#[tool(skip)]` instead.

### `#[wrap]` — a curated subset of a client's methods

```rust,no_run
use tokitai::wrap;

#[wrap(client = reqwest::Client, methods = [get_user, list_repos])]
impl GitHubClient { /* methods with their bodies */ }
```

Avoid when the method set changes often — `methods = […]` must
be re-issued on every new method. Prefer `#[tool]` with
`#[tool(skip)]` on helpers in that case.

### `#[openapi]` + `#[openapi_op]` — vendored OpenAPI spec

Use when you have an OpenAPI 3 JSON spec and want a
`phf::Map<operationId, Op>` for fast runtime lookup. Avoid for
large specs (500+ operations) — the `phf::Map` adds 100+ KB to
`.rodata` ([ADR-0002](adr/0002-phf-map-for-openapi-ops.md));
use `#[wrap]` with `methods = […]` for tighter budgets.

### `#[delegate]` — forward to an inner method

```rust,no_run
use tokitai::delegate;

impl OpenAIClient {
    #[delegate(to = "self.inner")]
    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, OpenAIError>;
}
```

`#[delegate]` does **not** emit a `call_tool` dispatcher or a
`ToolProvider` impl — it is meant to be combined with `#[tool]`
or used standalone with a manual dispatcher. See
[`wrap-architecture.md` §4.3](wrap-architecture.md#43-delegateto--).

---

## Composing resilience macros

The three resilience decorators — `#[retry]`, `#[rate_limit]`,
`#[circuit_breaker]` — all rewrite the body of a
`Result`-returning function. They stack; **the order matters**.

### The rule: innermost = most-restrictive

```text
#[retry]               ← ③ outermost: re-runs the whole call on Err
#[circuit_breaker]     ← ② middle:   short-circuits if the dep is broken
#[rate_limit]          ← ① innermost:throttles every call
async fn call(&self) -> Result<_, _> { /* body */ }
```

Why this order: `#[rate_limit]` is **always** evaluated (even
on retry), so it must be innermost. `#[circuit_breaker]`
should be **outside** the rate-limit so a rate-limited call
does not get retried (rate-limit is rejection, not failure).
`#[retry]` should be **outside** the circuit-breaker so a
transient `Err` that doesn't open the circuit still gets
retried.

```rust,no_run
// Wrong: rate-limit wraps retry — a 3-attempt retry eats 3× the budget
#[rate_limit(rps = 5)]
#[retry(max = 3, backoff = "exponential")]
async fn call(&self) -> Result<_, _> { /* … */ }
```

### Worked example and limits

```rust,no_run
use tokitai::{tool, retry, rate_limit, circuit_breaker};

#[tool]
impl WeatherClient {
    /// Fetch the weather for a city, with retries and a soft cap on QPS.
    #[retry(max = 3, backoff = "exponential", jitter = true)]
    #[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]
    #[rate_limit(rps = 5, burst = 10)]
    pub async fn get_weather(&self, city: String) -> Result<Weather, Error> {
        self.http.get(format!("https://api.weather.example/{city}"))
            .send().await?
            .json().await
    }
}
```

* As of T-004 (0.5.2), the inter-attempt sleep on `async fn` is
  driven by `tokitai_core::async_sleep(...)`, which yields to
  whatever executor is in scope (Tokio, async-std, smol, ...) and
  never blocks the calling runtime worker thread. Registering an
  `AsyncExecutor` is recommended for hot paths but no longer
  required to avoid the runtime-blocking `std::thread::sleep`
  fallback
  ([ADR-0001](adr/0001-async-executor-type-erasure.md)).
* Nested `#[retry]` in v1 do not stack cleanly — the inner
  layer wins.
* `#[circuit_breaker]` v1 is **observe-only** — it records
  state but does not short-circuit the body
  ([ADR-0004](adr/0004-circuit-breaker-v1-observe-only.md)).
* No extra runtime deps — built on `std::sync::atomic` and
  `std::time::SystemTime` only.
---

## Parameter design

A tool's parameter list is its **schema** — the LLM uses it
to decide which arguments to pass, and `#[tool]` uses it to
emit the JSON Schema. Treat it as the **public API to the
LLM**, not an internal detail.

### Good vs bad

```rust,no_run
// Bad: opaque, optional-everything, no validation
pub fn search(&self, data: serde_json::Value) -> Vec<String> { /* … */ }

// Good: named, typed, validated, documented
pub fn search(
    &self,
    #[tool(min_length = 3, max_length = 200)] query: String,
    #[tool(min = 1, max = 100, default = 20)] limit: Option<u32>,
) -> Result<Vec<Hit>, SearchError> { /* … */ }
```

What the LLM sees: `data: Value` (opaque JSON) vs
`query: string (3–200 chars)`, `limit: integer? (1–100, default 20)`.

### Seven rules

1. **Descriptive names.** `user_id`, `max_results`,
   `created_after`. Never `data`, `args`, `obj`, `x` — schema
   property names are the LLM's only handle on the argument.
2. **Concrete types over `serde_json::Value`.** A `Value`
   parameter gets a schema of `{}`.
3. **`Option<T>` only when genuinely optional.** Required
   parameters are not `Option<T>`; the macro marks them as
   `required: [...]`.
4. **Validate at the schema, not the body.** A
   `#[tool(min_length = 3, max_length = 200)]` guard gives the
   LLM a schema it can see *and* runs at ~10 ns per check.
5. **`default` for sensible defaults.** A
   `#[tool(default = 20)]` on `Option<u32>` puts the default in
   the JSON Schema.
6. **`example` for the trickiest cases.** A
   `#[tool(example = "\"2026-01-15T00:00:00Z\"")]` teaches the
   LLM the expected format.
7. **Document in doc comments, not `#[tool(desc = "…")]`**. The
   doc comment becomes both the tool's `description` field and
   the IDE docstring.

### What makes a bad parameter

* `String` for things that should be enums (`enum_values_status =
  ["active", "archived"]` instead).
* `Vec<String>` for things that should be `Vec<u32>` (the LLM
  will pass strings).
* `serde_json::Value` for everything (the schema is `{}`).
* `Option<Vec<T>>` for absent-or-non-empty (use `Vec<T>` with
  `min_items = 0, default = []`).
* Generic parameters: `T: Serialize` — not supported; the build
  will fail.

---

## Doc comment hygiene

`#[tool]` parses the doc comment on each `pub` method and
turns it into the `description` field of the `ToolDefinition`.
The doc comment serves two audiences with the same text — the
LLM (which gets the `description`) and the Rust developer
(which sees it in `cargo doc` / IDE hover). Write for the LLM
first, then make sure the text reads well to a human. See
[`USAGE.md` §Three ways to describe a tool](USAGE.md#three-ways-to-describe-a-tool).

### The `@`-directive syntax

`#[tool]` recognises `@`-prefixed directives in the doc comment:

| Directive                       | What it does                              | Example                                          |
|---------------------------------|-------------------------------------------|--------------------------------------------------|
| `@param <name> <description>`   | Override the parameter description         | `@param id the unique user identifier`           |
| `@validate <name> <constraint>` | Inline validation (alt to `#[tool(…)]`)   | `@validate id min=1, max=9999999`                |
| `@transform <name> <expr>`      | Apply a transformation before validation  | `@transform email lowercase`                     |
| `@example <json>`               | Example input in the schema                | `@example {"id": 42, "include_profile": true}`  |
| `@default <json>`               | Default value in the schema                | `@default null` or `@default []`                 |

`@param` is the most useful. `@example` and `@default` are
short-hands for `#[tool(example_X = "…")]` and
`#[tool(default_X = "…")]`. `@validate` and `@transform` are
legacy — prefer the `#[tool]` attribute form for new code.

---

## Error handling

`#[tool]` makes your error type a first-class citizen. If the
return type is `Result<T, E>`, the wrapper returns
`Ok(serde_json::to_value(v).unwrap())` on success and
`Err(ToolError::InternalError(format!("{}", e)))` on error. If
the return type is bare `T`, the wrapper synthesises a
`serde_json::to_value(t).unwrap()` — there is no `Err` to
surface.

### Supported shapes

```rust,no_run
use tokitai::ToolError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("not found")] NotFound,
    #[error("bad input: {0}")] BadInput(String),
    #[error("internal: {0}")] Internal(String),
}

// Good: Bare T  — infallible:  pub fn add(&self, a: i32, b: i32) -> i32 { a + b }
// Good: Result<T, ToolError>:   pub fn lookup(&self, id: i32) -> Result<User, ToolError> { … }
// Good: Result<T, MyError>:     pub fn lookup2(&self, id: i32) -> Result<User, MyError> { … }
// Bad:  Result<T, anyhow::Error>: LLM sees the Display impl, often a multi-line backtrace.
```

### What the LLM sees — and how to override

The MCP server's `POST /call` returns HTTP 200 with
`{ "success": false, "error": "..." }` (see
[`CROSS_LANGUAGE.md` §5](CROSS_LANGUAGE.md#5-error-handling)).
HTTP status codes are reserved for protocol-level errors;
tool-level failures are 200 + `success: false`. The `error`
string is `format!("{}", e)` of your error — make it
**specific and actionable** for `LLM-recoverable` failures:

```text
Bad:  "internal error"
Good: "user_id must be a positive integer (got -1)"
Good: "rate limit exceeded; retry in 30s"
Good: "tool 'add' is deprecated; use 'sum' instead"
```

The default wrapper uses
`ToolError::InternalError(format!("{}", e))`. For a
user-recoverable `ValidationError` or a richer message, return
`ToolError` directly from your method:

```rust,no_run
use tokitai::ToolError;

#[tool]
impl UserService {
    pub fn get_user(&self, id: i32) -> Result<User, ToolError> {
        if id < 0 {
            return Err(ToolError::validation_error(
                "id must be non-negative"));
        }
        // …
    }
}
```

For a method-level `error_message` override, see
[`reference/tool.md`](reference/tool.md).

---

## Testing your tools

Tokitai tools are regular Rust types; they test like any other
Rust type. The macro generates `call_tool` and a `ToolProvider`
impl on the same type, so you can test both the typed API and
the dispatcher API in the same test module.

### The basic test

```rust,no_run
use tokitai::{tool, ToolProvider};
use serde_json::json;

#[tool]
impl Calculator { pub fn add(&self, a: i32, b: i32) -> i32 { a + b } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_typed()   { assert_eq!(Calculator.add(2, 3), 5); }
    #[test] fn test_dispatch() {
        let v = Calculator.call_tool("add", &json!({"a": 2, "b": 3})).unwrap();
        assert_eq!(v, json!(5));
    }
    #[test] fn test_defs() {
        let defs = Calculator::tool_definitions();
        assert_eq!(defs[0].name, "add");
        assert!(defs[0].input_schema.contains("\"a\""));
    }
}
```

### Concurrency

The dispatcher is `&self`-only and the `LazyLock` is
`&'static`, so `Arc<MyType>` is enough — no `Mutex` needed.

### In-tree pattern and LLM round-trip

`tokitai/tests/wrap_smoke_test.rs` is the canonical example: a
small `MyApi` (5 methods), a `MyWrap`, a `DelegateHolder`, and
an `OpenApiClient`. It asserts `tool_count` matches, each tool
is reachable by name, and async methods emit both `__call_X`
and `__call_X_sync`.

For the LLM round-trip, use a deterministic mock or Ollama —
see `examples/ollama_integration.rs` and
`examples/multi_tool_chat.rs`.
---

## Anti-patterns

Seven things that look like a good idea but are not.

### A1. Returning `serde_json::Value`

A `Value` return is `{}` in the schema — the LLM cannot reason
about it. A concrete return is a precise schema the LLM can
describe in its reasoning trace.

```rust,no_run
// Bad:  pub fn search(&self, q: String) -> serde_json::Value { … }
// Good:
#[derive(serde::Serialize)] pub struct SearchResult { pub hits: Vec<Hit>, pub total: u32 }
pub fn search(&self, q: String) -> Result<SearchResult, SearchError> { … }
```

### A2. `Option<T>` for required parameters

`Option<T>` produces `required: false` in the schema. The LLM
treats the parameter as optional and may omit it. If the
parameter is required, declare it as `T`.

```rust,no_run
// Bad:  pub fn create_user(&self, name: Option<String>, email: Option<String>) -> User { … }
// Good: pub fn create_user(&self, name: String, email: String) -> Result<User, UserError> { … }
```

### A3. Validation in the body, not the schema

A `#[tool(min_length = 3)]` guard runs at ~10 ns per check
([`performance.md` §Runtime](performance.md#runtime)) and
gives the LLM a schema it can see. Hand-rolled `if name.len() < 3`
in the body is faster but loses the schema.

```rust,no_run
// Bad:  pub fn create_user(&self, name: String) -> Result<User, String> {
//     if name.len() < 3 { return Err("name too short".into()); }
// }
// Good: pub fn create_user(#[tool(min_length = 3, max_length = 50)] name: String) -> Result<User, UserError> { … }
```

### A4. Exposing internal helpers

Every `pub` method in a `#[tool]` impl becomes a tool the LLM
can call.

```rust,no_run
// Bad:  pub fn validate(&self, u: &User) -> bool { … }  // not a tool!
// Good: #[tool(skip)] pub fn validate(&self, u: &User) -> bool { … }
```

### A5. Generic methods

The `#[tool]` macro does not support generic methods. The
proc-macro cannot synthesise a monomorphised wrapper; the build
will fail with "Generic methods are not supported". Use
concrete types — name the methods after the type.

```rust,no_run
// Bad:  pub fn encode<T: serde::Serialize>(&self, value: T) -> String { … }
// Good: pub fn encode_string(&self, value: String) -> String { … }
```

### A6. One giant `#[tool]` impl

Incremental `cargo check` cost is dominated by *which* file you
touched. A change to one domain re-checks that impl's `fn`
items, not the 204 of a monolithic 100-method block.

```rust,no_run
// Bad:  #[tool] impl MegaClient { /* … 100 methods … */ }
// Good: #[tool] impl UserClient  { /* … 25 methods … */ }
//          #[tool] impl OrderClient { /* … 25 methods … */ }  // etc.
```

### A7. Forgetting `#[tool(skip)]` on `pub` tests or fixtures

Every `pub` method is a tool. A `_test_helper` or `_internal`
method that is `pub` (perhaps for in-crate testing) is exposed
to the LLM. Either make it not `pub`, or add `#[tool(skip)]`
explicitly.

```rust,no_run
// Bad:  pub fn _test_helper(&self) -> bool { … }  // pub, so it's a tool
// Good: fn _test_helper(&self) -> bool { … }      // not pub, not a tool
```

---

## See also

* [`performance.md`](performance.md) — companion guide.
* [`wrap-architecture.md`](wrap-architecture.md),
  [`wrap-cheatsheet.md`](wrap-cheatsheet.md) — wrap features.
* [`docs/reference/`](reference/) — per-attribute reference.
* [`docs/adr/`](adr/) — the six ADRs.
* [`docs/tutorials/getting-started.md`](tutorials/getting-started.md)
  — tutorial.
* [`docs/quickstart.md`](quickstart.md), [`docs/USAGE.md`](USAGE.md),
  [`docs/ADVANCED_USAGE.md`](ADVANCED_USAGE.md) — narrative.
* [`docs/AI_INTEGRATION.md`](AI_INTEGRATION.md),
  [`docs/CROSS_LANGUAGE.md`](CROSS_LANGUAGE.md) — integration.
