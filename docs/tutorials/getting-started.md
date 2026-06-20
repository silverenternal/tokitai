# Getting Started with Tokitai

Welcome! **Tokitai** is a Rust proc-macro library that turns the methods of
a Rust `impl` block into AI-callable tools *at compile time*. You drop a
`#[tool]` attribute on an `impl` block, and the macro generates the JSON
Schema, the dispatcher, and the [`ToolCaller`] implementation for you —
type errors surface during compilation, not when the LLM picks a bad
argument at runtime.

This tutorial is the first thing to read after `cargo add tokitai`. It
walks you through five progressive chapters:

| #  | Chapter                                  | What you'll build                                     |
|----|------------------------------------------|-------------------------------------------------------|
| 1  | Hello, tool                              | A `Calculator` with one `add` method exposed to AI    |
| 2  | Multiple tools + parameter validation    | A multi-method `Calculator` with constraints          |
| 3  | Async tools and the runtime-agnostic bridge | An async `WeatherClient` driven from a sync context |
| 4  | Resilience decorators                    | A flaky service wrapped with retry / rate-limit / circuit-breaker |
| 5  | Wrapping a third-party API               | A wrapped `OpenAIClient` with `#[wrap]` and `#[delegate]` |

Every chapter is **standalone**: you can read them in any order, copy
the code into a fresh `src/main.rs`, and run it. By the end of chapter
5 you'll have a production-grade, agent-ready Rust service.

## Prerequisites

- **Rust 1.80 or newer** (Tokitai's MSRV).
- A working `cargo` toolchain.

Add Tokitai to `Cargo.toml`:

```toml
[dependencies]
tokitai = "0.6"
serde_json = "1"
```

> If you only need the `#[tool]` codegen and want to hand-roll the
> runtime yourself, depend on it without defaults:
> `tokitai = { version = "0.6", default-features = false }`.

## Table of contents

1. [Chapter 1 — Hello, tool](#chapter-1--hello-tool)
2. [Chapter 2 — Multiple tools + parameter validation](#chapter-2--multiple-tools--parameter-validation)
3. [Chapter 3 — Async tools and the runtime-agnostic bridge](#chapter-3--async-tools-and-the-runtime-agnostic-bridge)
4. [Chapter 4 — Resilience decorators](#chapter-4--resilience-decorators)
5. [Chapter 5 — Wrapping a third-party API](#chapter-5--wrapping-a-third-party-api)
6. [Where to go next](#where-to-go-next)

---

## Chapter 1 — Hello, tool

In this chapter you'll expose a single Rust method as a tool that an
LLM can call. We'll write a `Calculator` struct with an `add` method,
mark it with `#[tool]`, and then call it via the [`ToolCaller`] API.

### The code

Save this as `src/main.rs`:

```rust,ignore
use tokitai::{tool, ToolProvider, ToolCaller, json};

#[derive(Default)]
pub struct Calculator;

#[tool]
impl Calculator {
    /// Add two 32-bit integers and return the result.
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

fn main() {
    let calc = Calculator;

    // Step 1: inspect the generated tool definitions.
    // `tool_definitions` returns `&'static [ToolDefinition]` — the
    // schema is baked in at compile time.
    for def in Calculator::tool_definitions() {
        println!(
            "Tool: {} -- {}",
            def.name,
            def.description.as_deref().unwrap_or("(no description)")
        );
    }

    // Step 2: call the tool. `call_tool` parses the args, runs the
    // method, and serialises the result back to `serde_json::Value`.
    let result = calc
        .call_tool("add", &json!({ "a": 10, "b": 20 }))
        .expect("add should succeed");
    println!("add(10, 20) = {}", result);

    println!("Chapter 1 complete");
}
```

Run it:

```text
$ cargo run
Tool: add -- Add two 32-bit integers and return the result.
add(10, 20) = 30
Chapter 1 complete
```

### What just happened

The `#[tool]` attribute on the `impl` block walked each method and
generated, at compile time:

- A function `__TOOL_DEF_ADD()` returning the tool's JSON Schema
  (a `ToolDefinition` struct with `name`, `description`, and a
  stringified `input_schema`).
- A wrapper function `__call_add(args)` that parses the args, calls
  `self.add(...)`, and serialises the result.
- A `call_tool(name, args)` dispatcher that matches on the tool name.
- `impl ToolProvider for Calculator` (the `tool_definitions` source)
  and `impl ToolCaller for Calculator` (the dispatcher source).

None of this code is in your source file — it's all synthesised by
the proc-macro at compile time. The doc comment above `add` was
harvested to become the `description` field of the tool, which is
what an LLM sees when it decides whether to call the tool.

### Try it

- Add a `subtract` method, rebuild, and call it. Watch a new
  `Tool: subtract` line appear with no extra glue.
- Add `#[tool(name = "sum", desc = "Sum of two ints")]` above the
  method to override the auto-derived name and description.
- Add `#[tool_min(0)]` on a parameter and call the tool with a
  negative number to see parameter validation kick in
  (chapter 2 covers this in detail).

---

## Chapter 2 — Multiple tools + parameter validation

A real agent has a whole toolkit, not a single function. In this
chapter we'll expose a `Calculator` with several methods, each carrying
its own description and parameter constraints. You'll learn how to
shape the JSON Schema that the LLM sees — without writing any JSON
by hand.

### The code

Save this as `src/main.rs`:

```rust,ignore
use tokitai::{tool, ToolProvider, ToolCaller, json};

#[derive(Default)]
pub struct Calculator;

#[tool]
impl Calculator {
    /// Add two integers and return the result.
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Multiply two integers and return the result.
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }

    /// Divide `dividend` by `divisor`. Returns an error string if
    /// `divisor` is zero. Both arguments must be non-negative.
    pub fn divide(
        &self,
        #[tool_min(0)] dividend: i32,
        #[tool_min(0)] divisor: i32,
    ) -> Result<i32, String> {
        if divisor == 0 {
            Err("division by zero".into())
        } else {
            Ok(dividend / divisor)
        }
    }

    /// Echo a message after validating it looks like a slug
    /// (lowercase letters, digits, and dashes only).
    pub fn slugify(
        &self,
        #[tool_pattern(r"^[a-z0-9-]+$")] input: String,
    ) -> String {
        input
    }

    /// Look up a weather report for a city. The argument is required
    /// (no `Option<...>`), and the macro enforces that explicitly.
    pub fn get_weather(
        &self,
        #[tool_required]
        #[tool_min_length(1)]
        #[tool_max_length(64)]
        city: String,
    ) -> String {
        format!("(stub) weather for {city}")
    }
}

fn main() {
    // 1. The macro generated one definition per `pub` method.
    let defs = Calculator::tool_definitions();
    println!("registered {} tools:", defs.len());
    for d in defs {
        println!("  - {}: {}", d.name, d.description);
    }

    // 2. Print the JSON schema of one tool to inspect constraints.
    let divide = Calculator::find_tool("divide").expect("divide exists");
    println!("\nschema for `divide`:\n{}", divide.input_schema);

    // 3. Drive the tools by name.
    let calc = Calculator;
    let r = calc.call_tool("add", &json!({"a": 2, "b": 3})).unwrap();
    println!("add -> {r}");

    // Calling with a negative value should fail at the schema layer.
    match calc.call_tool("divide", &json!({"dividend": -1, "divisor": 2})) {
        Ok(v) => println!("unexpected ok: {v}"),
        Err(e) => println!("divide(-1, 2) rejected: {e:?}"),
    }

    // A valid slug round-trips.
    let r = calc
        .call_tool("slugify", &json!({"input": "hello-world-42"}))
        .unwrap();
    println!("slugify -> {r}");

    println!("Chapter 2 complete");
}
```

### What just happened

Three new ideas landed in this chapter:

1. **Doc comments become descriptions.** The first `///` line on each
   method is harvested into the `description` field of the
   [`ToolDefinition`]. This is the string the LLM reads when it
   decides whether to call the tool — write it as if you were
   writing a docstring for a junior developer.

2. **Parameter attributes become schema constraints.** The macro
   understands a small DSL of `#[tool_*]` attributes on parameters:
   - `#[tool_min(N)]` / `#[tool_max(N)]` — numeric bounds.
   - `#[tool_min_length(N)]` / `#[tool_max_length(N)]` — string length.
   - `#[tool_pattern(r"...")]` — regex (anchored; the macro
     serialises it as a JSON Schema `pattern`).
   - `#[tool_required]` — explicit "this argument is required" even
     if the LLM omits it (most useful with `Option<T>` to make the
     schema unambiguous).
   - `#[tool_default = "expr"]` — default value when the LLM omits it.

3. **`Result<T, E>` becomes a tool error.** When your method returns
   `Result<T, String>` (or any `E: Display`), the `Err` arm is
   converted into a [`ToolError`] of kind `ExecutionError` and the
   `call_tool` returns it. This is how you tell the LLM that a
   call failed for *domain* reasons, as opposed to schema
   *validation* reasons.

### Try it

- Add `#[tool_min_length(3)]` to the `input` parameter of `slugify`
  and call it with `"hi"`. Watch the schema reject the call.
- Add a `Result<i32, String>`-returning `modulo` method that errors
  on `divisor == 0` and confirm the error is surfaced through
  `call_tool`.
- Use `#[tool(name = "lookup_weather", desc = "...")]` to rename
  `get_weather` and check the new name in `tool_definitions`.

---

## Chapter 3 — Async tools and the runtime-agnostic bridge

Almost every interesting tool — fetching a URL, calling an LLM,
querying a database — is async. In this chapter you'll expose an
async method, drive it from a sync context, and learn how Tokitai
decouples itself from Tokio via the [`AsyncExecutor`] trait.

> This chapter assumes you've added a Tokio dependency to
> `Cargo.toml`:
> ```toml
> [dependencies]
> tokitai = "0.6"
> serde_json = "1"
> tokio = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
> ```

### The code

Save this as `src/main.rs`:

```rust,ignore
use std::time::Duration;
use tokitai::tool;
use tokitai::{ToolProvider, ToolCaller, json, ToolError};

/// A tiny "weather" client whose `fetch` is async — it pretends to
/// make a network call.
#[derive(Default)]
pub struct WeatherClient {
    /// Each call bumps a counter, so the demo can prove the body
    /// actually ran.
    pub calls: std::sync::atomic::AtomicU32,
}

#[tool]
impl WeatherClient {
    /// Fetch a one-line weather summary for `city`. Async because
    /// in a real client this would do an HTTP request.
    pub async fn fetch(&self, city: String) -> Result<String, String> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // Pretend we just did a network round-trip.
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(format!("{city}: sunny, 22°C"))
    }
}

fn main_sync() -> Result<(), ToolError> {
    // We're a *sync* function calling an *async* tool. The macro
    // synthesised a sync wrapper that drives the future on the
    // current Tokio runtime (or any executor you registered with
    // `set_async_executor` — see below).
    let client = WeatherClient::default();
    let result = client.call_tool("fetch", &json!({ "city": "Tokyo" }))?;
    println!("sync call: {result}");
    println!("Chapter 3 complete");
    Ok(())
}

#[tokio::main]
async fn main() {
    // === Path A: from inside a Tokio runtime ===
    //
    // When the call site is already async, you don't *need* the
    // bridge — you can call the method directly. But it's often
    // more uniform to go through `call_tool`, and the macro wires
    // it up for you.

    let client = WeatherClient::default();
    let r = client
        .call_tool("fetch", &json!({ "city": "Paris" }))
        .expect("paris call");
    println!("async-context call: {r}");

    // === Path B: from a sync context, via the bridge ===
    //
    // `call_tool` is sync. The macro-generated wrapper drives the
    // underlying async function on a registered executor (or, by
    // default, the active Tokio handle). If you want to be
    // runtime-agnostic — e.g. to plug `async-std`, `smol`, or your
    // own executor — install one with `set_async_executor`.
    main_sync().expect("sync path works");
}
```

### What just happened

Two things to internalise:

1. **Async methods become sync-on-the-dispatcher.** The `#[tool]`
   macro inspects the method signature. When the method is
   `async fn`, the generated `call_tool` path *does not* require
   you to be in an async context: it transparently drives the
   future through the [`block_on_async`] helper, which:

   1. Uses your registered [`AsyncExecutor`] if any.
   2. Falls back to the active Tokio `Handle::block_on`.
   3. Returns a clear `ToolError` otherwise.

   The point is that **the LLM-facing surface stays sync** even
   when the implementation is async. This is what lets you mix
   sync and async tools behind a single dispatcher.

2. **`AsyncExecutor` is the escape hatch.** If you don't want to
   pull in Tokio — say, you're on `embassy`, `async-std`, or a
   custom executor — implement [`AsyncExecutor`] once and register
   it at startup:

   ```rust,ignore
   use tokitai_core::{set_async_executor, AsyncExecutor, AsyncExecutorExt};
   use core::pin::Pin;

   struct BlockingExecutor;
   impl AsyncExecutor for BlockingExecutor {
       fn block_on_dyn(
           &self,
           future: Pin<Box<dyn core::future::Future<Output = ()> + Send>>,
       ) -> Box<dyn core::any::Any + Send> {
           Box::new(futures::executor::block_on(future))
       }
   }

   set_async_executor(Box::new(BlockingExecutor));
   ```

   The first call to `set_async_executor` wins; subsequent calls
   are silently ignored. After registration, every
   `#[tool]`-generated sync wrapper will route through *your*
   executor instead of Tokio.

### Try it

- Remove the `#[tokio::main]` annotation, install a
  `BlockingExecutor` via `set_async_executor`, and call `fetch`
  from a plain `fn main()`. The same `call_tool` line should
  work without any async context.
- Add `#[tokio::test)]` test that calls `fetch` directly (the
  async version, not `call_tool`) and check the `calls`
  counter — this exercises the "we're already async" path.
- Wrap the call in `tokio::task::spawn_blocking(...)` to confirm
  the bridge is happy to drive a future from a blocking thread
  as long as a Tokio handle is in scope.

---

## Chapter 4 — Resilience decorators

Most tools you'll expose hit the network. Networks are flaky, rate
limits exist, and downstream services go down. The
`#[retry]`, `#[rate_limit]`, and `#[circuit_breaker]` attributes
let you bolt production-grade fault-tolerance onto a tool method
*at compile time* — no runtime config, no `tower::Service` trait
plumbing, no manual back-off loops.

### The code

Save this as `src/main.rs`:

```rust,ignore
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
// Resilience decorators are re-exported from the `tokitai` umbrella crate,
// so end users don't need to depend on `tokitai-macros` directly.
use tokitai::{circuit_breaker, rate_limit, retry, tool, ToolCaller, ToolError, ToolProvider, json};

/// A small service that demonstrates all three resilience decorators.
#[derive(Default)]
pub struct ResilientService {
    /// Public counter so we can assert that retry actually re-ran the body.
    pub fetch_count: AtomicU32,
}

#[tool]
impl ResilientService {
    /// Fetch a URL, retrying transient failures with exponential
    /// backoff and jitter. The decorator re-wraps the body — the
    /// signature you see is exactly what the AI sees.
    #[retry(max = 3, backoff = "exponential", jitter = true)]
    pub async fn fetch_url(&self, url: String) -> Result<String, String> {
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        if url.is_empty() {
            return Err("empty url".into());
        }
        Ok(format!("fetched: {url}"))
    }

    /// Emit a log line, throttled to 10 messages per second with
    /// a burst of 20. The decorator adds an atomic CAS at the
    /// top of the body — there's no async involved.
    #[rate_limit(rps = 10, burst = 20)]
    pub fn log_event(&self, message: String) -> String {
        format!("logged: {message}")
    }

    /// Call an external service behind a circuit breaker that opens
    /// after 5 consecutive failures and re-probes after 30 seconds.
    /// (Composition with `#[retry]` is shown below.)
    #[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]
    pub async fn call_external(&self, endpoint: String) -> Result<String, String> {
        if endpoint.is_empty() {
            return Err("missing endpoint".into());
        }
        Ok(format!("called {endpoint}"))
    }

    /// Stack decorators: the breaker decides whether to allow the
    /// call at all, the rate limiter throttles the allowed calls,
    /// and the retry handles the remaining transient failures.
    /// Read attributes top-down: the **outer** attribute wraps the
    /// **inner** one.
    #[retry(max = 3, backoff = "linear", jitter = false)]
    #[rate_limit(rps = 5, burst = 5)]
    #[circuit_breaker(failure_threshold = 3, reset_timeout = "10s")]
    pub async fn protected_call(&self, target: String) -> Result<String, String> {
        if target == "fail" {
            return Err("synthetic failure".into());
        }
        Ok(format!("ok: {target}"))
    }
}

fn main() -> Result<(), ToolError> {
    // 1. All four methods are registered as tools with the same
    //    shape they would have without the decorators.
    let svc = ResilientService::default();
    let defs = svc.tool_definitions();
    println!("registered {} tools:", defs.len());
    for d in defs {
        println!("  - {}", d.name);
    }

    // 2. Drive a retry from a sync context (same bridge as chapter 3).
    let r = svc.call_tool("fetch_url", &json!({ "url": "https://example.com" }))?;
    println!("fetch_url -> {r}");
    assert_eq!(svc.fetch_count.load(Ordering::SeqCst), 1);

    // Calling with an empty url fails *after* 3 attempts.
    let r = svc.call_tool("fetch_url", &json!({ "url": "" }));
    println!("fetch_url(empty) -> {r:?}");
    assert_eq!(svc.fetch_count.load(Ordering::SeqCst), 4);

    // 3. The rate-limited sync method works the same way.
    let r = svc.call_tool("log_event", &json!({ "message": "hi" }))?;
    println!("log_event -> {r}");

    // 4. A protected call succeeds.
    let r = svc.call_tool("protected_call", &json!({ "target": "ok" }))?;
    println!("protected_call -> {r}");

    println!("Chapter 4 complete");
    Ok(())
}
```

### What just happened

Three new attributes and one composition rule:

| Attribute                | What it does                                                 | Generated extras                          |
|--------------------------|--------------------------------------------------------------|-------------------------------------------|
| `#[retry(max, backoff, jitter)]` | Re-invokes the body on `Result::Err`, with backoff          | rewrites the body                         |
| `#[rate_limit(rps, burst)]`      | Token-bucket throttle using one atomic CAS per call         | rewrites the body (lock-free)             |
| `#[circuit_breaker(threshold, reset)]` | Three-state breaker (closed → open → half-open)        | rewrites the body (3 atomics, lock-free)  |

The **composition rule** is the one thing to remember:

> **The outer attribute wraps the inner one.**

In the `protected_call` example, attribute order is
`#[retry]` → `#[rate_limit]` → `#[circuit_breaker]`, but the
*call* order at runtime is the reverse: the breaker is the
outermost gate, then the rate limiter, then the body, and the
retry loop is the innermost layer that re-runs the body on
failure. Read attributes **top-down, call flow bottom-up** — see
[`docs/wrap-cheatsheet.md`](../wrap-cheatsheet.md) for the
canonical diagram.

All three decorators operate *on the body* of your function,
not on the dispatcher. The generated `__call_<NAME>` wrapper
remains identical to a non-decorated tool: same JSON Schema,
same `call_tool` signature, same `ToolDefinition`. That's the
whole point — resilience is a property of the implementation,
not of the contract.

> For the full deep-dive on composition order, parameter
> semantics, and limitations, see
> [`docs/wrap-architecture.md`](../wrap-architecture.md).

### Try it

- Drop the `#[retry]` from `protected_call`, call it with
  `target: "fail"` in a loop, and watch the circuit open after
  three failures.
- Reduce `rps = 1, burst = 1` on `log_event` and call it 100
  times in a tight loop. Some calls will return an "out of
  tokens" error — that's the rate limiter in action.
- Remove the `#[rate_limit]` from `protected_call` and confirm
  the failure behaviour of the bare `#[retry]` +
  `#[circuit_breaker]` stack.

---

## Chapter 5 — Wrapping a third-party API

By chapter 5 you can build a clean internal service. But real
agents usually have to talk to *someone else's* API: OpenAI,
GitHub, Stripe, an internal REST service. Tokitai gives you
three ways to expose a third-party client as a set of tools:

- `#[wrap]` — pre-select a curated list of methods on a client.
- `#[delegate]` — write *signatures only*; the macro injects
  forwarding bodies.
- `#[openapi]` + `#[openapi_op]` — vendored OpenAPI 3 spec
  drives the whole schema set.

In this chapter we'll wrap a tiny "OpenAI" client and compare
all three patterns.

### The code

Save the inner client and types as `src/inner.rs`:

```rust,ignore
//! A tiny stand-in for a real OpenAI client. In production this
//! would be a `reqwest::Client` wrapper, an `openai` SDK, etc.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
}

#[derive(Default)]
pub struct InnerOpenAIClient {
    pub calls: std::sync::atomic::AtomicU32,
}

impl InnerOpenAIClient {
    pub fn chat(&self, req: ChatRequest) -> Result<ChatResponse, String> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(ChatResponse {
            text: format!("(stub) {} on {}", req.messages.join(" "), req.model),
        })
    }

    pub fn list_models(&self) -> Vec<Model> {
        vec![
            Model { id: "gpt-4o".into() },
            Model { id: "gpt-4o-mini".into() },
        ]
    }
}
```

Then `src/main.rs`:

```rust,ignore
mod inner;
use inner::{ChatRequest, InnerOpenAIClient};
use tokitai::{delegate, wrap, ToolProvider, ToolCaller, json, ToolError};

/// A wrapped OpenAI client using `#[wrap]`. The macro generates
/// a `pub fn new(client: InnerOpenAIClient) -> Self` constructor
/// and only exposes the methods listed in `methods = [...]`.
pub struct OpenAIWrapClient {
    pub client: InnerOpenAIClient,
}

#[wrap(client = InnerOpenAIClient, methods = [chat, list_models])]
impl OpenAIWrapClient {
    /// Send a chat completion request to the OpenAI API.
    pub fn chat(&self, req: ChatRequest) -> Result<inner::ChatResponse, String> {
        self.client.chat(req)
    }

    /// List the model ids available to this account.
    pub fn list_models(&self) -> Vec<inner::Model> {
        self.client.list_models()
    }
}

/// A forwarded client using `#[delegate]`. Here we only write
/// *signatures* — the macro injects `self.inner.chat(req)` etc.
/// There is no auto-generated `call_tool` dispatcher with
/// `#[delegate]` alone, so we wire one up by hand below.
pub struct OpenAIDelegateClient {
    pub inner: InnerOpenAIClient,
}

impl OpenAIDelegateClient {
    /// Forward to `self.inner.chat(req)`.
    #[delegate(to = "self.inner")]
    pub fn chat(&self, req: ChatRequest) -> Result<inner::ChatResponse, String>;

    /// Forward to `self.inner.list_models()`.
    #[delegate(to = "self.inner")]
    pub fn list_models(&self) -> Vec<inner::Model>;
}

fn main() -> Result<(), ToolError> {
    // ===== #[wrap] path: one attribute, full ToolProvider =====

    let wrap_client = OpenAIWrapClient::new(InnerOpenAIClient::default());
    let defs = OpenAIWrapClient::tool_definitions();
    println!("wrap: {} tools registered", defs.len());
    for d in &defs {
        println!("  - {}", d.name);
    }

    let r = wrap_client.call_tool(
        "chat",
        &json!({
            "req": { "model": "gpt-4o", "messages": ["hi"] }
        }),
    )?;
    println!("wrap chat -> {r}");

    // ===== #[delegate] path: signatures only, hand-rolled dispatcher

    let del_client = OpenAIDelegateClient {
        inner: InnerOpenAIClient::default(),
    };

    // The macro emitted `__call_chat` / `__call_list_models` wrappers
    // for us. We dispatch by name from a small match block.
    let r = dispatch(&del_client, "chat", &json!({
        "req": { "model": "gpt-4o", "messages": ["hello"] }
    }))?;
    println!("delegate chat -> {r}");

    // ===== Which one should I use? =====

    // Use `#[wrap]` when you have a *client struct* (reqwest,
    // redis, an SDK) and want to curate which methods become tools.
    // Use `#[delegate]` when the *forwarding target* is a
    // free-form expression (a method chain, an associated fn, etc.)
    // and you don't need the auto-generated dispatcher.
    // Use `#[openapi]` when you have a vendored OpenAPI 3 spec and
    // want the schema set driven entirely by the spec.

    println!("Chapter 5 complete");
    Ok(())
}

/// Hand-rolled dispatcher over the `__call_*` wrappers emitted
/// by `#[delegate]`. In a real `#[tool]` impl block the macro
/// generates this for you; here we write it explicitly to make
/// the connection between the attribute and the dispatcher clear.
fn dispatch(
    client: &OpenAIDelegateClient,
    name: &str,
    args: &tokitai::Value,
) -> Result<tokitai::Value, ToolError> {
    match name {
        "chat" => OpenAIDelegateClient::__call_chat(client, args),
        "list_models" => OpenAIDelegateClient::__call_list_models(client, args),
        other => Err(ToolError::not_found(other)),
    }
}
```

### What just happened

Three patterns, three different ergonomics:

1. **`#[wrap(client = T, methods = [...])]`** — the
   "I have a client struct" pattern. Drop the attribute on an
   `impl` block, list exactly the methods you want exposed, and
   the macro generates:
   - A `pub fn new(client: T) -> Self` constructor that wires up
     the inner client.
   - A `ToolProvider::tool_definitions` returning one definition
     per listed method.
   - A `ToolCaller::call_tool` dispatcher.
   - A `tool_count()` accessor (handy for assertions).

   This is what you reach for most of the time when wrapping a
   third-party client.

2. **`#[delegate(to = "self.inner")]`** — the
   "I have a free-form expression" pattern. Write *signatures
   only* (no `{}` body), point `to` at any expression, and the
   macro injects `<to>.<method_name>(<args>)`. It emits the
   schema and the `__call_*` wrapper, but **not** a dispatcher
   — you bring your own. Useful when the forward target isn't
   a field (e.g. `Config::default()`, a method chain, or
   something that doesn't fit the `client` field shape that
   `#[wrap]` requires).

3. **`#[openapi(spec = "...", base_url = "...")]` + `#[openapi_op(operation_id = "...")]`** —
   the "I have a spec" pattern. The macro reads an OpenAPI 3
   spec at proc-macro compile time (via `include_str!`) and
   generates a `phf::Map` keyed by `operationId`. Zero runtime
   spec parsing. You write a method body that actually makes
   the HTTP call, and the macro glues it to the right
   `operationId`. See [`examples/wrap_openapi.rs`](../../examples/wrap_openapi.rs)
   for a complete OpenAI-via-OpenAPI walkthrough.

> For the full taxonomy, the "when to use which" decision tree,
> and the OpenAPI compile-time pipeline diagram, see
> [`docs/wrap-architecture.md`](../wrap-architecture.md).

### Try it

- Add a third method to `OpenAIWrapClient` (say, `embed`) but
  *don't* list it in `methods = [...]`. Rebuild and check
  `OpenAIWrapClient::tool_count()` — it's still 2.
- Add a `#[retry(max = 3)]` to the `chat` method on
  `OpenAIWrapClient` and confirm the wrap + retry stack
  composes (chapter 4's outer-wraps-inner rule applies here
  too).
- Take an OpenAPI 3 spec for a service you use, drop it next
  to `Cargo.toml` as `my_api.json`, and try replacing the
  `#[wrap]` impl with `#[openapi(spec = "my_api.json",
  base_url = "...")]` + `#[openapi_op(operation_id = "...")]`
  on each method.

---

## Where to go next

You've now seen the full Tokitai toolkit end-to-end: a single
sync method in chapter 1, a constrained multi-tool service in
chapter 2, an async tool driven through a runtime-agnostic
bridge in chapter 3, a resilience-decorated service in chapter
4, and a third-party wrapper in chapter 5. Here's where to go
from here:

- **[`docs/wrap-architecture.md`](../wrap-architecture.md)** —
  the long-form reference. Read this for the compile-time
  pipeline diagram, the resilience composition rules, and the
  v1 limitations. **You should not duplicate this in your own
  code**; it's the authoritative source.
- **[`docs/wrap-cheatsheet.md`](../wrap-cheatsheet.md)** — a
  one-page summary of every attribute, where it goes, and what
  it generates. Bookmark it.
- **[`docs/ARCHITECTURE.md`](../ARCHITECTURE.md)** — high-level
  crate topology (`tokitai`, `tokitai-core`, `tokitai-macros`,
  `tokitai-mcp-server`).
- **[`examples/`](../../examples/)** — runnable examples for
  every shipped feature, including `basic_usage`, `multi_tool_chat`,
  `mcp_server_demo`, `ollama_integration`, `dev_assistant`, and
  `wrap_openapi`. `wrap_native`, `delegate_method`, and
  `resilient_tool` are placeholders under
  [`examples/deprecated/`](../../examples/deprecated/) because their
  proc-macro attributes are not part of v0.6.0.
- **[API reference on docs.rs](https://docs.rs/tokitai)** — the
  full rustdoc, including [`ToolProvider`], [`ToolCaller`],
  [`ToolDefinition`], [`ToolError`], and the [`AsyncExecutor`]
  family.
- **[Cross-language guide](../CROSS_LANGUAGE.md)** — driving
  your Tokitai-defined tools from Python, JavaScript, Go, or
  curl via the MCP HTTP server.

If you build something with Tokitai, open an issue or a PR on
the [repository](https://github.com/silverenternal/tokitai) —
we'd love to hear about it.
