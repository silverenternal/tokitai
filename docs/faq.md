# Tokitai — FAQ & Troubleshooting Guide

**Crate versions covered**: 0.6.0 (latest)
**Audience**: downstream users of [`tokitai`](https://crates.io/crates/tokitai)
who hit a compile error, a runtime error, a confusing tool definition, or
a behaviour gap between the docs and the macro output.

If you have read [`tutorials/getting-started.md`](tutorials/getting-started.md)
and the relevant page in [`reference/`](reference/README.md) and still
have a question, the answer is almost certainly here.

---

## 1. Quick links — the 10 most-asked questions

1. **My tool doesn't appear in `tool_definitions()`.** [Why → §4.1](#41-why-does-my-tool-not-appear-after-i-add-it)
2. **The LLM always sends `null` for my `Option<T>` parameter.** [Why → §4.2](#42-why-does-the-llm-always-send-null-for-an-optiont-parameter)
3. **My async tool fails to compile with "future is not `Send`".** [Why → §4.3](#43-why-does-my-async-tool-fail-to-compile-with-future-is-not-send)
4. **I get `TOOL_DEFINITIONS` from older code — the macro has changed.** [Fix → §3.1](#31-error-rust-cannot-find-value-tool_definitions-in-this-scope)
5. **I get `compile_error!: Generic methods are not supported`.** [Fix → §3.2](#32-error-compile_error--tool-method-name-uses-unsupported-generic-parameters)
6. **`#[openapi]` says it cannot find the spec file at compile time.** [Fix → §3.6](#36-error-openapi-could-not-read-spec-file-path)
7. **`#[rate_limit]` blocks my Tokio worker.** [Why → §4.4](#44-why-does-rate_limit-block-my-tokio-worker)
8. **The body of my `#[delegate]` method gets rejected.** [Fix → §3.4](#34-error-delegate-is-meant-to-be-applied-to-a-method-signature-no-body)
9. **My custom struct shows up as `{"type":"object"}` with no fields.** [Fix → §4.5](#45-why-does-my-custom-struct-show-up-as-typeobject-with-no-fields)
10. **`config!` doesn't seem to override anything.** [Why → §4.6](#46-why-doesnt-config-actually-override-my-tool-description)

---

## 2. Macro-specific FAQ

Eleven entry points, eleven focused sections. Each Q&A is "Q: ...\nA: ..."
with a minimal, copy-pasteable code example. Every example is compatible
with the v0.4 / v0.5 source tree.

### 2.1 `#[tool]`

> The workhorse attribute. Goes on an `impl` block; every `pub` method
> becomes an AI tool. See [`reference/tool.md`](reference/tool.md) for the
> full table of arguments.

#### Q: Does `#[tool]` see private methods?

A: No. Only `pub` methods are exposed. Anything else (including
`pub(crate)`, `pub(super)`, and methods without a visibility modifier)
is invisible to the macro and is not registered. Use `#[tool(skip)]`
to explicitly exclude a `pub` method you don't want exposed.

```rust
use tokitai::tool;

#[derive(Default)]
pub struct Service;

#[tool]
impl Service {
    /// This IS a tool.
    pub fn public_method(&self) -> i32 { 1 }

    // NOT a tool — not `pub`.
    fn private_helper(&self) -> i32 { 0 }

    /// Also NOT a tool — explicitly excluded.
    #[tool(skip)]
    pub fn internal(&self) -> i32 { -1 }
}
```

#### Q: Can I rename a method as seen by the LLM?

A: Yes. Use the method-level `#[tool(name = "...")]`. The Rust name
and the tool name can differ; the dispatcher matches on the tool name.

```rust
use tokitai::tool;

#[derive(Default)]
pub struct Weather;

#[tool]
impl Weather {
    #[tool(name = "fetch_weather", desc = "Fetch weather from external API")]
    pub fn get_weather(&self, city: String) -> String { city }
}

// Callers use the renamed tool name, not the Rust name:
let w = Weather;
let r = w.call_tool("fetch_weather", &serde_json::json!({"city":"Paris"}))
    .unwrap();
```

#### Q: My method has a generic parameter — the macro rejects it. Workaround?

A: `#[tool]` does not support generic methods (the generated
`__call_<name>` would need monomorphisation hints that aren't in
the method signature). Use concrete types, or split the generic
helper out of the `#[tool]` impl.

```rust
use tokitai::tool;

#[derive(Default)]
pub struct Tools;

#[tool]
impl Tools {
    // ERROR: `Generic methods are not supported`
    // pub fn process<T: serde::Serialize>(&self, v: T) -> String { ... }

    // Concrete-typed wrappers:
    pub fn process_string(&self, v: String) -> String { v }
    pub fn process_json(&self, v: serde_json::Value) -> String { v.to_string() }

    // Generic helper stays non-public:
    fn process_inner<T: serde::Serialize>(&self, v: &T) -> String {
        serde_json::to_string(v).unwrap_or_default()
    }
}
```

#### Q: Why does `call_tool` return a `ToolError` of kind `InternalError` even when my method succeeded?

A: It doesn't — `InternalError` (kind = 2) means the method returned
`Result::Err`, or the runtime result-serialisation step failed. A
clean return is `Ok(value)`. Use the kind to branch:

```rust
use tokitai::{ToolError, ToolErrorKind, json};

let calc = MyTools::default();
match calc.call_tool("divide", &json!({"a": 10, "b": 0})) {
    Ok(v) => println!("ok: {v}"),
    Err(ToolError { kind: ToolErrorKind::ValidationError, message }) =>
        eprintln!("bad arg: {message}"),
    Err(ToolError { kind: ToolErrorKind::NotFound, message }) =>
        eprintln!("no such tool: {message}"),
    Err(ToolError { kind: ToolErrorKind::InternalError, message }) =>
        eprintln!("tool returned Err: {message}"),
    Err(ToolError { kind: ToolErrorKind::TypeError, message }) =>
        eprintln!("type mismatch: {message}"),
}
```

#### Q: How do I see what JSON Schema the macro generated?

A: Two ways. Either call `input_schema_value()` on a `ToolDefinition`,
or use `cargo expand` to see the macro output inline.

```rust
use tokitai::ToolProvider;

let td = MyTools::find_tool("divide").expect("divide exists");
let schema: serde_json::Value = serde_json::from_str(&td.input_schema).unwrap();
println!("{}", serde_json::to_string_pretty(&schema).unwrap());
```

Or:

```bash
cargo install cargo-expand
cargo expand --lib | less
```

---

### 2.2 `#[tool_type]`

> Attach a hand-written JSON Schema to a custom struct so it doesn't
> fall back to a bare `{"type":"object"}`. See
> [`reference/tool-type.md`](reference/tool-type.md).

#### Q: My struct's schema is empty `{}`. What went wrong?

A: The `properties = "..."` shorthand in `#[tool_type]` is parsed by
a tiny ad-hoc parser (see the [`reference`](reference/tool-type.md#arguments)
table for accepted types). Anything outside `string`, `integer`, `number`,
`boolean`, `array`, `object` is silently treated as `Any` and dropped.
Split complex fields into a nested `#[tool_type]` instead.

```rust
use tokitai::tool_type;

#[tool_type(
    name = "User",
    properties = "id: integer, name: string, address: object, tags: array",
    required = "id, name",
)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub address: Option<String>, // nested struct? use a separate `#[tool_type]`
    pub tags: Vec<String>,
}
```

#### Q: Does the `name = "..."` have to match the Rust type name?

A: It is the lookup key into `tokitai_core::TYPE_SCHEMA_CACHE`. The
macro auto-registers the schema under the *Rust* ident, so for a
plain `pub struct User { … }` the `name = "User"` is redundant but
harmless. Use a different `name` only if you have a reason to (e.g.
you want the same schema registered under two different keys).

#### Q: Can I have two `#[tool_type]` blocks with the same `name`?

A: The cache is a process-global `BTreeMap<String, JsonSchema>`. The
second registration silently overwrites the first. There is no
dead-config warning. The macro tracks a [strict-mode proposal in
issue #42](https://github.com/silverenternal/tokitai/issues/42).

---

### 2.3 `config!`

> Runtime override of tool descriptions, tags, and per-parameter
> hints. See [`reference/config.md`](reference/config.md).

#### Q: Does `config!` replace the `#[tool]` attribute, or stack with it?

A: Stacks. The override layer is applied at first access of
`tool_definitions()` (via `__get_tool_definitions`'s `LazyLock`).
Priority order is: doc comment → `#[tool(...)]` →
`config!` (last write wins). `config!` is the right tool for tweaking
descriptions without recompiling the `#[tool]` impl block.

```rust
use tokitai::config;

config! {
    WeatherService {
        get_weather: {
            desc: "Fetch current weather for a city",
            tags: ["weather", "read-only"],
            params: {
                city: { desc: "City name", example: "Tokyo" }
            }
        }
    }
}
```

#### Q: I called `config!` and the LLM still sees the old description.

A: Two causes. (a) You called `MyType::tool_definitions()` *before*
`config!` ran — the `LazyLock` cached the pre-override list. Force a
re-read with `MyType::configure_tool(name, &[])` if you need a
re-init. (b) You used `#[openapi]` — that path explicitly **ignores**
`config!` because the spec is the source of truth.

#### Q: Does `config!` work with `#[wrap]` or `#[delegate]`?

A: Yes for `#[wrap]` (it shares the `__get_tool_definitions` shape
with `#[tool]`). No for `#[delegate]` — `#[delegate]` does not emit
a `__get_tool_definitions` function, so there is nothing for the
registry to apply to.

---

### 2.4 `#[wrap]`

> Same as `#[tool]` but with a curated `methods = [...]` list and a
> generated `new(client)` constructor. See
> [`reference/wrap.md`](reference/wrap.md).

#### Q: I get `method <name> listed in methods = [...] was not found in the impl block`.

A: The macro requires the method to be present **and** `pub` in the
impl block. Renamed a method? Update the `methods = [...]` list.
Made a method `pub(crate)` to silence another warning? The wrap
attribute still wants `pub`.

```rust
use tokitai::wrap;

pub struct InnerClient;

pub struct Wrapper { pub client: InnerClient }

#[wrap(client = InnerClient, methods = [ping])]   // listed
impl Wrapper {
    pub fn ping(&self) -> bool { true }
    pub fn health(&self) -> bool { true }         // not listed → not a tool
}
```

#### Q: My struct doesn't have a `client` field — what now?

A: Pass `field = "..."` to tell the macro which field to wire into
the generated `new()` constructor.

```rust
use tokitai::wrap;

pub struct Http;

pub struct Api { pub http: Http }

#[wrap(client = Http, methods = [do_thing], field = "http")]
impl Api {
    pub fn do_thing(&self) -> String { "ok".into() }
}

let api = Api::new(Http);
```

---

### 2.5 `#[openapi]` and `#[openapi_op]`

> Drive a `ToolProvider` from an OpenAPI 3 spec file. See
> [`reference/openapi.md`](reference/openapi.md). The spec is read at
> *proc-macro compile time* (resolved via `Span::local_file()` — see
> [ADR-0006](adr/0006-openapi-spec-path-resolution.md)) and baked into
> a `phf::Map` ([ADR-0002](adr/0002-phf-map-for-openapi-ops.md)).

#### Q: My spec is YAML. What do I do?

A: `#[openapi]` accepts JSON only. Convert with `yq`:

```bash
yq -o=json '.spec' openai_chat.yaml > openai_chat.json
```

Then point the attribute at the converted file.

#### Q: My method has no `#[openapi_op]` attribute — does it become a tool?

A: No. Without `#[openapi_op(operation_id = "...")]` the method is
silently skipped, even if it is `pub`. The `#[openapi]` attribute
opts each method in independently.

```rust
use tokitai::{openapi, openapi_op};

#[openapi(spec = "openai_chat.json", base_url = "https://api.openai.com/v1")]
impl OpenAIClient {
    #[openapi_op(operation_id = "createChatCompletion")]
    pub async fn create_chat_completion(&self, body: ChatRequest)
        -> Result<ChatResponse, reqwest::Error> { /* ... */ }

    // Not a tool, even though pub.
    pub async fn helper(&self) -> u32 { 0 }
}
```

#### Q: Does `#[openapi]` honour `config!` overrides?

A: No. `#[openapi]` generates a no-op `configure_tool` and the
`__get_tool_definitions` it emits does **not** consult
`GLOBAL_CONFIG_REGISTRY`. OpenAPI-derived metadata is treated as
the source of truth.

---

### 2.6 `#[delegate]`

> Write a method **signature only**; the macro injects the body.
> See [`reference/delegate.md`](reference/delegate.md).

#### Q: Can I add my own body to a `#[delegate]` method?

A: No. `#[delegate]` requires the method to be a *signature*
(declared with `;`, no `{}`). To add logic, write a real method or
chain through a helper:

```rust
use tokitai::delegate;

pub struct Inner;
impl Inner {
    pub fn ping(&self) -> bool { true }
}

pub struct Wrapper { pub inner: Inner }

impl Wrapper {
    #[delegate(to = "self.inner")]
    pub fn ping(&self) -> bool;     // signature only

    // ERROR: body present.
    // #[delegate(to = "self.inner")]
    // pub fn ping(&self) -> bool { false }
}
```

#### Q: My `#[delegate]` impl block doesn't expose a `call_tool` dispatcher.

A: That is by design. `#[delegate]` emits `__TOOL_DEF_*` and
`__call_*` items but no `call_tool` dispatcher (so it can compose
freely with `#[tool]` and `#[wrap]`). Wire one up by hand:

```rust
use tokitai::delegate;
use tokitai::ToolDefinition;

impl Wrapper {
    #[delegate(to = "self.inner")]
    pub fn ping(&self) -> bool;
}

fn dispatch(c: &Wrapper, name: &str, args: &serde_json::Value)
    -> Result<serde_json::Value, tokitai::ToolError>
{
    match name {
        "ping" => Wrapper::__call_ping(c, args),
        other => Err(tokitai::ToolError::not_found(other)),
    }
}

fn definitions(c: &Wrapper) -> Vec<ToolDefinition> {
    vec![Wrapper::__TOOL_DEF_PING().clone()]
}
```

#### Q: My delegate target is a method chain, not a field. Will `#[delegate]` work?

A: Yes. The `to` argument is parsed as a `syn::Expr`, so any Rust
expression type-checks in the method's context. The generated body
becomes `<to>.<method_name>(<args>)` (or just `<to>` for associated
functions with no `&self`).

```rust
use tokitai::delegate;

pub struct OpenAIClient { pub inner: OpenAISdk }

impl OpenAIClient {
    #[delegate(to = "self.inner")]                   // field
    pub async fn chat(&self, req: ChatRequest)
        -> Result<ChatResponse, OpenAIError>;

    #[delegate(to = "Config::default()")]            // associated fn
    pub fn default_config() -> Config;
}
```

---

### 2.7 `#[retry]`

> Re-runs the body of a `Result`-returning function on `Err`, with
> backoff. See [`reference/retry.md`](reference/retry.md).

#### Q: My `#[retry]` method doesn't seem to retry.

A: Two things to check. (a) The function must return a `Result`. The
macro's match-arm is a `Result` match; if your function returns
`Option<T>`, the `Ok` arm always wins and the loop runs once. (b)
Count the `max` — the first attempt counts, so `max = 3` means
**three total attempts**, not three retries on top of the first.

```rust
use tokitai::tool;
use tokitai_macros::retry;

#[tool]
impl WeatherClient {
    /// Up to **3** total attempts, exponential backoff with jitter.
    #[retry(max = 3, backoff = "exponential", jitter = true)]
    pub async fn get_weather(&self, city: String) -> Result<Weather, String> {
        // body that may return Err(...)
    }
}
```

#### Q: Can I retry on a *specific* error, not all `Err`?

A: Not in v0.5. The `on` argument is accepted for forward
compatibility, but v0.6.0 always retries on any `Err`. If you need
conditional retry, branch inside the body and return a non-retryable
error for the "do not retry" case.

---

### 2.8 `#[rate_limit]`

> Lock-free token-bucket throttle. See
> [`reference/rate-limit.md`](reference/rate-limit.md).

#### Q: The first call after idle is throttled, not the burst.

A: It isn't — but the *refill math* is `now - last_ns >= interval`.
If the runtime clock jumps backward (NTP, suspend/resume), the
refill reads as zero tokens and the call throttles. Use a monotonic
clock for the time source if your runtime supports it.

#### Q: Does `rps = 0` mean "unlimited"?

A: No. The macro silently clamps `rps = 0` to `rps = 1` (one call
per second). The same applies to `burst = 0`. If you want "no
rate limit", don't decorate the function.

---

### 2.9 `#[circuit_breaker]`

> 3-state breaker (closed / open / half-open). See
> [`reference/circuit-breaker.md`](reference/circuit-breaker.md) and
> the design rationale in
> [ADR-0004](adr/0004-circuit-breaker-v1-observe-only.md).

#### Q: My circuit "opens" but the body still runs. Is the macro broken?

A: Not broken — v1 is **observe-only**. The macro records the state
in three static atomics and updates the public counters, but it does
**not** short-circuit the call. This is by design (see
[ADR-0004](adr/0004-circuit-breaker-v1-observe-only.md)); it avoids
forcing every decorated function to have an error type that
implements `From<String>`. If you want fail-fast, read
`__CB_STATE` at the top of the body:

```rust
use std::sync::atomic::Ordering;
use tokitai_macros::circuit_breaker;

#[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]
async fn fast_fail(&self) -> Result<String, String> {
    // 1 = Open. The macro emits a `pub(crate)`-visible `__CB_STATE`.
    if Self::__CB_STATE.load(Ordering::Relaxed) == 1u8 {
        return Err("circuit open".to_string());
    }
    Ok("ok".to_string())
}
```

v2 will add a `CircuitOpen` trait so the macro can synthesise the
fail-fast error itself.

#### Q: My `reset_timeout = "5m"` parses to zero. Why?

A: Check the suffix. Accepted suffixes are `ms`, `s`, `m`, `h`, or a
bare integer (interpreted as seconds). A typo (`"5min"`,
`"5 minutes"`) silently becomes `0`, which means the breaker
transitions to half-open on the very next call.

---

### 2.10 `param_tool!` and the per-parameter attributes

> The 16 `#[tool_*]` attributes plus their bundled form
> `#[param_tool(...)]`. See [`reference/param-attrs.md`](reference/param-attrs.md).

#### Q: What's the difference between `#[tool_min = 0]` on a parameter and `#[tool(min_x = 0)]` on the method?

A: They produce the same JSON Schema entry. Use whichever you find
more readable:

```rust
use tokitai::tool;

#[tool]
impl Signup {
    // (1) Per-parameter attribute — closest to the binding it constrains.
    pub fn create_a(&self, #[tool_min = 0] age: i32) -> i32 { age }

    // (2) Method-level key — good when the same constraint applies to many params.
    #[tool(min_age = 0, max_age = 150)]
    pub fn create_b(&self, age: i32) -> i32 { age }
}
```

#### Q: I tried `#[param_tool(...)]` and got "cannot find macro `param_tool` in this scope".

A: It's exported as `tokitai::param_tool`, **not** as a
function-like macro. The `!` after the name is a misnomer carried
over from older drafts; `param_tool` is a *bundled attribute*:

```rust
use tokitai::{tool, param_tool};

#[tool]
impl Account {
    pub fn open(&self,
        #[param_tool(min = 0, max = 150, desc = "User age")]
        age: i32,
    ) -> i32 { age }
}
```

---

## 3. Error-message lookup

Paste your error message; find the cause and fix. Every entry below
is something the proc-macro actually emits (verified against
[`reference/`](reference/) and the source comments in
`tokitai-macros/src/tool/*`).

### 3.1 `error: cannot find value `TOOL_DEFINITIONS` in this scope`

**Cause**: pre-0.4 code accessing the const that 0.4 replaced with
a method. See the v0.4 changelog entry in [`CHANGELOG.md`](../CHANGELOG.md).

**Fix**: switch to the method form.

```rust
// old (pre-0.4)
let tools = MyTools::TOOL_DEFINITIONS;

// new (0.4+)
let tools = MyTools::tool_definitions();
```

### 3.2 `error: compile_error! tool method <name> uses unsupported generic parameters`

**Cause**: `#[tool]` does not support generic methods. See
[`reference/tool.md` §Errors](reference/tool.md#errors).

**Fix**: use concrete types.

```rust
use tokitai::tool;

#[tool]
impl Tools {
    // pub fn process<T: serde::Serialize>(&self, v: T) -> String { ... }

    pub fn process_string(&self, v: String) -> String { v }
    pub fn process_json(&self, v: serde_json::Value) -> String { v.to_string() }
}
```

### 3.3 `error: duplicate definitions with name `__call_<method>``

**Cause**: this was a v0.4 bug on impl blocks that mixed sync and
async methods. Fixed in v0.5; the fix is included in
[`migration/v0.4-to-v0.5.md`](migration/v0.4-to-v0.5.md#bug-fix-1-tool-mixed-syncasync-methods).

**Fix**: upgrade to 0.5 (recommended) or split sync and async
methods into separate `#[tool]` impl blocks.

```toml
[dependencies]
tokitai = "0.6"   # was "0.4"
```

### 3.4 `error: #[delegate] is meant to be applied to a method signature (no body); remove the existing method body`

**Cause**: a `#[delegate]` attribute on a method that has a `{ ... }`
body. `#[delegate]` synthesises the body itself.

**Fix**: drop the body and end the signature with `;`.

```rust
use tokitai::delegate;

pub struct Inner;
impl Inner { pub fn ping(&self) -> bool { true } }

pub struct W { pub inner: Inner }

impl W {
    // #[delegate(to = "self.inner")]
    // pub fn ping(&self) -> bool { false }

    //
    #[delegate(to = "self.inner")]
    pub fn ping(&self) -> bool;
}
```

### 3.5 `error: missing `client = TYPE` in `#[wrap(...)]`` / `methods = [...]` must list at least one method`

**Cause**: `#[wrap]` requires both `client` and `methods`. The `methods`
list must be non-empty (use `#[tool]` if you want every public method).

**Fix**:

```rust
use tokitai::wrap;

pub struct Inner;

pub struct W { pub client: Inner }

#[wrap(client = Inner, methods = [ping])]   // both present, non-empty
impl W {
    pub fn ping(&self) -> bool { true }
}
```

### 3.6 `error: #[openapi] could not read spec file <path>: <io error>`

**Cause**: the spec path is resolved relative to the source file via
`Span::local_file()` ([ADR-0006](adr/0006-openapi-spec-path-resolution.md)),
and either the file is not at that path or `local_file()` returned
`None` (which happens when the proc-macro server can't see the
source on disk — e.g. some custom build systems).

**Fix**: put the spec next to your `.rs` file, or pass an absolute
path.

```rust
use tokitai::openapi;

#[openapi(spec = "openai_chat.json", base_url = "https://api.openai.com/v1")]
impl OpenAIClient { /* ... */ }
```

```text
.
├── Cargo.toml
├── openai_chat.json   <-- drop it here
└── src
    └── lib.rs
```

### 3.7 `error: operation_id <id> not found in OpenAPI spec <path>`

**Cause**: the `operation_id` in `#[openapi_op]` does not match any
key in the spec's `paths.*.operationId` map.

**Fix**: print every `operationId` the macro saw, then copy-paste:

```rust
use tokitai::{openapi, openapi_op, ToolProvider};

#[openapi(spec = "openai_chat.json", base_url = "https://api.openai.com/v1")]
impl OpenAIClient {
    // After building, inspect __OPENAPI_OPS_OpenAIClient for the keys.
    #[openapi_op(operation_id = "createChatCompletion")]
    pub async fn create_chat_completion(&self, body: ChatRequest)
        -> Result<ChatResponse, reqwest::Error> { /* ... */ }
}
```

### 3.8 `error[E0433]: failed to resolve: use of undeclared crate or module `serde_json``

**Cause**: `serde` feature is off. The default `Cargo.toml` of
`tokitai` is `features = ["serde"]`, so this happens when you
explicitly opt out with `default-features = false`.

**Fix**:

```toml
[dependencies]
tokitai = { version = "0.4", features = ["serde"] }
serde_json = "1"
```

### 3.9 `error: failed to bind async function to call_tool: no executor registered`

**Cause**: the macro's sync-from-async bridge
([ADR-0003](adr/0003-sync-from-async-via-block-on-dyn.md)) needs an
`AsyncExecutor`. The fallback path needs you to be inside a Tokio
runtime; if neither is true, the call returns a `ToolError` of kind
`InternalError` with the message "an async tool was called but no
async executor is reachable".

**Fix**: either run inside a Tokio runtime, or register a custom
executor.

```rust
use std::pin::Pin;
use tokitai_core::{set_async_executor, AsyncExecutor};

struct BlockingExecutor;
impl AsyncExecutor for BlockingExecutor {
    fn block_on_dyn(
        &self,
        future: Pin<Box<dyn core::future::Future<Output = ()> + Send>>,
    ) -> Box<dyn core::any::Any + Send> {
        Box::new(futures::executor::block_on(future))
    }
}

// In main, before the first call_tool:
set_async_executor(Box::new(BlockingExecutor));
```

### 3.10 `warning: [tokitai] [W001] deprecated method <name> missing `replaced_by``

**Cause**: `#[tool(deprecated)]` on a method without a
`replaced_by = "..."` partner. See
[`reference/tool.md` §Errors](reference/tool.md#errors).

**Fix**: either add a `replaced_by`, or suppress per-method with
`allow = [...]`, or globally with `TOKITAI_QUIET=1`.

```rust
use tokitai::tool;

#[tool]
impl Calculator {
    #[tool(
        deprecated,
        replaced_by = "add_numbers",
        deprecated_note = "LLMs multiply by adding N times anyway.",
        deprecated_since = "0.5.0",
        remove_in = "0.7.0",
    )]
    pub async fn multiply(&self, a: i32, b: i32) -> i32 { a * b }
}
```

### 3.11 `warning: [tokitai] [W002] optional param <name> lacks default/example`

**Cause**: an `Option<T>` parameter with no `default_*` or
`example_*` set. The macro can't synthesise a hint for the LLM
about what to send when the value is missing.

**Fix**: add a default or an example. The default is the *schema*
default (what the LLM sees), not the Rust value — set both if you
care about runtime behaviour.

```rust
use tokitai::tool;

#[tool]
impl Search {
    /// Search with an optional limit.
    #[tool(
        default_limit = 10,
        example_limit = 50,
    )]
    pub fn search(&self, query: String, limit: Option<i32>) -> Vec<String> {
        // ...
        vec![]
    }
}
```

### 3.12 `warning: [tokitai] [W003] method <name> has `context="async"` but is not async`

**Cause**: you wrote `context = "async"` on a sync method.

**Fix**: make the method `async` (and adjust callers to `.await`),
or drop the `context` argument.

```rust
use tokitai::tool;

#[tool]
impl Calc {
    #[tool(context = "async")]
    pub async fn add(&self, a: i32, b: i32) -> i32 { a + b }   // matches
}
```

---

## 4. Common pitfalls

Ten things that surprise users, with the "why" and the right way.

### 4.1 Why does my tool not appear after I add it?

**Why**: either the method is not `pub`, the method is excluded
with `#[tool(skip)]`, the impl block lacks `#[tool]`, or — in the
case of `#[openapi]` — the method lacks `#[openapi_op]`.
`#[wrap]` and `#[tool_type]` have their own gatekeepers too.

**Right way**: print the registered names and compare.

```rust
use tokitai::ToolProvider;

for td in MyService::tool_definitions() {
    println!("  - {}", td.name);
}
```

If the method you expected is missing, walk the checklist:

```rust
use tokitai::tool;

#[tool]                                  // (1) impl-level attribute
impl MyService {
    pub fn visible(&self) -> i32 { 1 }   // (2) pub

    #[tool(skip)]
    pub fn hidden(&self) -> i32 { 2 }    // (3) explicitly excluded
}
```

### 4.2 Why does the LLM always send `null` for an `Option<T>` parameter?

**Why**: the LLM reads the schema. If the schema doesn't show a
`default`, an `example`, or a clear `description` for the
parameter, the LLM often passes `null` "to be safe". The macro
treats `null` as `None` correctly, but a richer schema cuts the
noise in half.

**Right way**: at minimum, give the parameter a `desc` and an
`example`. A `default` is even better.

```rust
use tokitai::tool;

#[tool]
impl Search {
    /// Search the docs.
    #[tool(
        desc_limit = "Maximum number of results to return",
        example_limit = 10,
        default_limit = 25,
    )]
    pub fn search(&self, query: String, limit: Option<i32>) -> Vec<String> {
        // ...
        vec![]
    }
}
```

### 4.3 Why does my async tool fail to compile with "future is not `Send`"?

**Why**: `#[tool]` generates a sync-from-async bridge whose
generated `__call_<name>_sync` wrapper holds a future. The
`async-trait` machinery inside the wrapper is `Send`, but if your
*body* contains a `Rc` / `RefCell` / `*mut` / non-`Send` type, the
whole future stops being `Send` and the Tokio worker pool rejects
it.

**Right way**: use `Arc<Mutex<T>>` instead of `Rc<RefCell<T>>`; avoid
`unsafe` non-`Send` pointers inside an `async` tool body; if you
absolutely need a `!Send` future, call the body from
`tokio::task::spawn_blocking` and `.await` the `JoinHandle`.

```rust
use tokitai::tool;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct State { pub count: Mutex<u32> }   // Send + Sync

#[tool]
impl Counter {
    pub async fn bump(&self, state: Arc<State>) -> u32 {
        let mut g = state.count.lock().unwrap();
        *g += 1;
        *g
    }
}
```

### 4.4 Why does `#[rate_limit]` block my Tokio worker?

**Why**: in pre-0.5.2 builds, the decorator fell back to
`std::thread::sleep` when no `AsyncExecutor` was registered.
`std::thread::sleep` on a Tokio worker thread blocks the whole
runtime. As of **T-004 (0.5.2)**, the resilience decorators on
`async fn` emit `await tokitai_core::async_sleep(...)`, which
yields to whatever executor is in scope (Tokio, async-std, smol,
...) and never blocks the runtime worker.

**Right way (still recommended)**: register a non-blocking
executor so the wait is driven by a real timer. The
[getting-started tutorial §3](tutorials/getting-started.md#chapter-3--async-tools-and-the-runtime-agnostic-bridge)
walks through this; the short version is:

```rust
use tokitai_core::{set_async_executor, AsyncExecutor};
use std::pin::Pin;

struct TokioExecutor;
impl AsyncExecutor for TokioExecutor {
    fn block_on_dyn(
        &self,
        future: Pin<Box<dyn core::future::Future<Output = ()> + Send>>,
    ) -> Box<dyn core::any::Any + Send> {
        let res = tokio::runtime::Handle::current().block_on(async move {
            future.await;
        });
        Box::new(res)
    }
}

fn main() {
    set_async_executor(Box::new(TokioExecutor));
    // ...your app...
}
```

If you depend on `tokitai` without the default features and add
`tokitai = { version = "0.4", features = ["serde", "tokio"] }`, the
generated code wires `Handle::block_on` directly. The above snippet
is what you need for `async-std`, `smol`, or any other runtime.

### 4.5 Why does my custom struct show up as `{"type":"object"}` with no fields?

**Why**: the auto-derived schema for a struct only carries the
field list if every field's type is recognised by the
`ParamType::from_rust_type` table (see
[`tokitai-core/src/lib.rs`](../../tokitai-core/src/lib.rs)). Newtype
wrappers and complex generics fall through to `object` and the
`properties` map is empty.

**Right way**: annotate the type with `#[tool_type]` and supply the
schema explicitly.

```rust
use tokitai::tool_type;

#[tool_type(
    name = "Location",
    properties = "latitude: number, longitude: number",
    required = "latitude, longitude",
)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}
```

### 4.6 Why doesn't `config!` actually override my tool description?

**Why**: either the impl block uses `#[openapi]` (which ignores
`config!` by design — see [`reference/config.md`](reference/config.md#errors)),
or you read `tool_definitions()` before `config!` ran and the
`LazyLock` cached the pre-override list.

**Right way**: place `config!` calls **before** the first
`tool_definitions()` call, or in static-init order, and double-check
the type name matches exactly.

```rust
use tokitai::{config, ToolProvider};

config! {
    WeatherService {
        get_weather: { desc: "Fetch current weather for a city" }
    }
}

fn main() {
    // This must be AFTER config! ran.
    let defs = WeatherService::tool_definitions();
    assert_eq!(defs[0].description, "Fetch current weather for a city");
}
```

### 4.7 Why does `#[tool]` reject my method that returns `Option<T>`?

**Why**: the dispatcher expects either `T` or `Result<T, E>`. An
`Option<T>` is a different shape and the auto-generated
`__call_<name>` wrapper doesn't know how to serialise it. The fix
is to `unwrap_or_default()` inside the body, or change the return
type to `Result<T, ()>` with `Ok(v) / Err(())`.

**Right way**:

```rust
use tokitai::tool;

#[derive(Default)]
pub struct Service;

#[tool]
impl Service {
    // pub fn maybe(&self) -> Option<i32> { Some(42) }

    // shape that the dispatcher knows:
    pub fn maybe(&self) -> i32 { 42 }
}
```

### 4.8 Why is my `#[wrap]` `new()` constructor missing?

**Why**: the macro emits `new` only when the impl block has a
field whose name matches the `client = ...` type. If your field
is named differently, you must pass `field = "..."` (or the
generated `new` will not compile because the macro tries to assign
`client` to a non-existent field).

**Right way**:

```rust
use tokitai::wrap;

pub struct Http;

pub struct Api { pub http: Http }   // not `client`

#[wrap(client = Http, methods = [do_thing], field = "http")]
impl Api {
    pub fn do_thing(&self) -> String { "ok".into() }
}

// let api = Api::new(Http);   // generated constructor
```

### 4.9 Why does my `#[openapi]` schema have empty `properties`?

**Why**: the spec file is valid JSON but not valid **OpenAPI 3**
JSON. The macro parses the top-level shape and walks `paths.*.*`
looking for `operationId` and parameters. If the spec is actually
a `swagger: "2.0"` doc, or a 3.1 doc with `type: ["…", "null"]`
arrays, the walker may produce empty parameter schemas.

**Right way**: convert Swagger 2.0 → 3.0 with
[`swagger2openapi`](https://www.npmjs.com/package/swagger2openapi)
before pointing `#[openapi]` at it. See the
[§7.2 limitations in `wrap-architecture.md`](wrap-architecture.md#72-openapi).

### 4.10 Why is my `#[retry]` stack "absorbing" my `#[rate_limit]`?

**Why**: attribute stacking is **outer-attribute-last**. The
topmost attribute wraps the body that already has the inner
attribute applied. So `#[retry] #[rate_limit] fn f()` means "the
retry loop wraps the rate-limited call", which is what you want
*only if* you want the rate limiter to throttle every attempt. If
you want one rate limit decision per *logical* call, swap the
order.

**Right way**: pick the order that matches the call flow you want.

```rust
use tokitai::tool;
use tokitai_macros::{retry, rate_limit};

#[tool]
impl Client {
    /// Throttle once per logical call; retry transient failures.
    #[retry(max = 3, backoff = "exponential", jitter = true)]
    #[rate_limit(rps = 10, burst = 20)]
    pub async fn fetch(&self, url: String) -> Result<String, String> {
        // ...
        Ok("ok".into())
    }
}
```

Read top-down, call flow bottom-up. See
[`wrap-architecture.md` §5](wrap-architecture.md#5-composition-rules).

### 4.11 Why does my `#[circuit_breaker]` test never observe an "open" state?

**Why**: v1 only transitions to `Open` after the **Nth** call's
`Err` arm executes *and* the call returns. If your test only
issues `Ok` calls, the breaker stays `Closed` (state `0`). Also,
the state atomics are per-function statics; if the test creates a
fresh service struct per call, the state still persists (statics
are not re-initialised), but if the binary has more than one
impl block with the same `circuit_breaker` decoration, each gets
its **own** atomics.

**Right way**: count and branch.

```rust
use std::sync::atomic::Ordering;
use tokitai::tool;
use tokitai_macros::circuit_breaker;

#[derive(Default)]
pub struct BreakerSvc;

#[tool]
impl BreakerSvc {
    #[circuit_breaker(failure_threshold = 3, reset_timeout = "1s")]
    pub async fn fail(&self) -> Result<(), String> {
        Err("nope".into())
    }
}

#[tokio::main]
async fn main() {
    let s = BreakerSvc::default;
    for _ in 0..3 {
        let _ = s.call_tool("fail", &serde_json::json!({})).await;
    }
    // After three Err returns, the breaker's static state is 1 (Open).
    // (We don't read it from outside the macro; v1 has no public API.)
}
```

### 4.12 Why is my tool name in the schema but the dispatcher returns `NotFound`?

**Why**: you renamed the tool with `#[tool(name = "...")]` (or the
LLM is calling an alias), but the alias is **not** in
`#[tool(alias = [...])]`. The dispatcher matches the **method
name** (or the `name` override), not the alias.

**Right way**: list the alias in `#[tool(alias = [...])]`.

```rust
use tokitai::tool;

#[tool]
impl Calculator {
    #[tool(
        name = "sum",
        alias = ["add", "plus", "add_two_numbers"],
        desc = "Add two numbers together",
    )]
    pub fn add(&self, a: i32, b: i32) -> i32 { a + b }
}
```

### 4.13 Why do my macro warnings flood the build log?

**Why**: W001 (`deprecated` without `replaced_by`), W002
(`Option<T>` without `default`/`example`), and W003
(`context = "async"` on a sync method) are warnings by default.

**Right way**: silence per-method with `allow = [...]`, or globally
with `TOKITAI_QUIET=1`. Set `TOKITAI_SHOW_WARNINGS=1` to re-enable
them (handy in `cfg(test)`). See
[`reference/tool.md` §Errors](reference/tool.md#errors).

```rust
use tokitai::tool;

#[tool]
impl Service {
    #[tool(allow = ["deprecated_missing_replaced_by"])]
    pub fn legacy(&self) -> i32 { 0 }
}
```

### 4.14 Why does `tool_definitions()` panic with "schema parse failed"?

**Why**: it doesn't panic — the multi-schema export methods
(`to_openai_function`, `to_anthropic_tool`, `to_mcp_tool`) return
an empty `parameters` object `{}` if the stored `input_schema` is
not valid JSON. The dispatcher itself never tries to parse the
schema; it just hands the raw `&'static str` to whoever wants it.
If a downstream consumer *does* try to parse and panics, that's a
bug in the consumer.

**Right way**: parse with `serde_json::from_str` and handle the
error.

```rust
use tokitai::ToolProvider;

let td = MyService::find_tool("x").unwrap();
match serde_json::from_str::<serde_json::Value>(&td.input_schema) {
    Ok(v)  => println!("ok: {v}"),
    Err(e) => eprintln!("schema parse failed: {e}"),
}
```

### 4.15 Why does my LLM see the wrong description even after I changed the doc comment?

**Why**: `cargo build` is incremental. The macro expansion for the
`#[tool]` impl block is cached; if the doc comment changed but the
file mtime didn't bump, the cache can serve the old expansion. Run
`cargo clean -p my-crate` once and rebuild.

---

## 5. Performance FAQ

#### Q: Is `#[tool]` faster than `#[openapi]`?

A: Yes — slightly. Both have the same dispatcher shape and the
same `__call_<name>` wrapper, so the per-call cost is identical
(see the bench numbers in [`CHANGELOG.md`](../CHANGELOG.md): ~150 ns
for a single-parameter sync call). The `#[openapi]` path adds one
`phf::Map` lookup **only at compile-time** to wire the per-method
artifacts. At runtime there is no difference. If you measure, the
gap is in the noise (single-digit ns).

The reason people perceive `#[tool]` as faster is that `#[openapi]`
expands to a larger `phf::Map` at compile time, which can slow
*compilation* by a few hundred milliseconds — not runtime. If you
care about cold-build times, prefer `#[tool]`; if you care about
runtime, it does not matter.

#### Q: When should I use `#[retry]` vs. writing a manual loop?

A: `#[retry]` is for cases where the body is small and the
backoff is fixed at compile time. Manual loops are better when:

- The number of attempts is runtime-configurable.
- The retry policy depends on the *kind* of error.
- You need to update a counter or metric between attempts.

`#[retry]` produces a tight `loop { match … }` with no
`Box::pin`, no `Future`, no `Mutex`. It is the lowest-overhead
option for a fixed retry policy.

#### Q: When should I use `#[rate_limit]` vs. `governor` / `tokio::time::interval`?

A: `#[rate_limit]` is for compile-time, per-function throttling
where the parameters are known up-front. For runtime-tunable limits
or shared limits across many functions, use `governor` (or a
similar crate) and a `DashMap` of buckets. The advantage of
`#[rate_limit]` is that it is **lock-free**: one `AtomicU32` and
one `AtomicU64` per decorated function, with a single
`compare_exchange` per call. See the
[performance table in `wrap-architecture.md`](wrap-architecture.md#6-performance-characteristics).

#### Q: When should I use `#[circuit_breaker]` vs. a hand-rolled `Mutex<State>`?

A: `#[circuit_breaker]` is a good default when the threshold and
the reset timeout are compile-time constants. It uses three
statics (`AtomicU8` for state, `AtomicU32` for failures,
`AtomicU64` for `open_at_ns`) and a single `fetch_add` on each
call. For dynamic thresholds or a per-tenant breaker, hand-roll a
`Mutex<CircuitState>` keyed by tenant.

#### Q: How can I make the first `call_tool` call fast?

A: `tool_definitions()` returns `&'static [ToolDefinition]` — the
schema is baked in. There is no init cost. The `__call_<name>`
wrapper is `#[inline]`-able, so the call is one `match` arm plus
`from_json_value` per parameter. On the bench, a 1-2 parameter
sync call is ~150 ns.

If you need to call an *async* tool from a sync context, the
`AsyncExecutor` registration matters: install a
`TokioExecutor` (see [§4.4](#44-why-does-rate_limit-block-my-tokio-worker))
at program startup, *before* the first `call_tool` is made. The
first call after registration is the same speed as later calls
because the executor is just a `&'static`.

#### Q: Does multi-schema export cost anything?

A: `to_openai_function`, `to_anthropic_tool`, and `to_mcp_tool`
each parse the `input_schema: String` once via
`serde_json::from_str` and wrap it in a `serde_json::json!`
envelope. Sub-microsecond per call. Cache the `Value` if you call
them in a hot path:

```rust
use tokitai::ToolProvider;

let defs = MyService::tool_definitions();
let openai_envelopes: Vec<serde_json::Value> =
    defs.iter().map(|d| d.to_openai_function()).collect();
```

---

## 6. Compatibility FAQ

| Question | Answer | Source |
|---|---|---|
| **Rust MSRV** | 1.80. Recorded in every `Cargo.toml` (`package.rust-version`). | [`tokitai/Cargo.toml`](../../tokitai/Cargo.toml) |
| **Edition** | 2021. | workspace `Cargo.toml` |
| **Async runtime** | Runtime-agnostic. Tokio is the default fallback; install a custom `AsyncExecutor` for `async-std` / `smol` / `embassy` / your own. | [ADR-0001](adr/0001-async-executor-type-erasure.md), [ADR-0003](adr/0003-sync-from-async-via-block-on-dyn.md) |
| **JSON Schema dialect** | Draft 2020-12 (a superset of the parts of Draft 7 that LLMs use). The macro emits `type`, `properties`, `required`, `minimum`/`maximum`, `minLength`/`maxLength`, `pattern`, `enum`, `multipleOf`, `default`, `example`, `description`. | [`reference/param-attrs.md`](reference/param-attrs.md) |
| **OpenAPI version** | **3.0** (JSON; convert YAML first). 3.1 is partial; Swagger 2.0 is not supported. | [`wrap-architecture.md` §4.2](wrap-architecture.md#42-openapi--openapi_op) |
| **MCP version** | The reference `tokitai-mcp-server` exposes a small HTTP+JSON subset (GET `/tools`, POST `/call`, GET `/health`, optional POST `/sse/<name>`). It is **not** the full JSON-RPC 2.0 spec used by every MCP implementation; see the cross-language guide for the wire format. | [`docs/CROSS_LANGUAGE.md`](CROSS_LANGUAGE.md) |
| **Provider envelopes** | OpenAI (`type: "function"`), Anthropic (`input_schema`), MCP (`inputSchema`). Use the three methods on `ToolDefinition`. | [`tokitai-core/src/lib.rs` §4.7](../../tokitai-core/src/lib.rs) |
| **Tokio version** | 1.x with the `full` feature. | workspace deps |
| **`serde` / `serde_json` version** | `serde` 1.0 (with `derive`); `serde_json` 1.0. | workspace deps |
| **`thiserror` version** | 1.0. | workspace deps |
| **`phf` version** | 0.11 (only used by `#[openapi]`). | [`tokitai-macros/Cargo.toml`](../../tokitai-macros/Cargo.toml) |
| **`async-trait` version** | 0.1. | workspace deps |
| **Stability of the `#[tool]` macro** | Stable across 0.4.x and 0.5.x. | [`docs/API_STABILITY.md`](API_STABILITY.md) |
| **Stability of the wrap features** | Stable as of 0.6.0 (per [`API_STABILITY.md`](API_STABILITY.md)). | — |
| **`circuit_breaker` v1 limitation** | Observe-only; no fail-fast. v2 will add it. | [ADR-0004](adr/0004-circuit-breaker-v1-observe-only.md) |
| **`call_tool_sync`/`call_tool` shape** | Sync for all-sync impls; async if any method is `async`. Mixed impl blocks were buggy in 0.4; fixed in 0.5. | [`migration/v0.4-to-v0.5.md`](migration/v0.4-to-v0.5.md#bug-fix-1-tool-mixed-syncasync-methods) |
| **Crate licence** | Dual MIT or Apache-2.0. | every `Cargo.toml` |
| **`#![no_std]`** | `tokitai-core` is `no_std`-compatible (with the `serde` feature off). `tokitai` and `tokitai-macros` are not. | [`tokitai-core/Cargo.toml`](../../tokitai-core/Cargo.toml) |

---

## 7. See also

### Reference (one page per attribute / macro)

- [`docs/reference/tool.md`](reference/tool.md) — `#[tool]`
- [`docs/reference/tool-type.md`](reference/tool-type.md) — `#[tool_type]`
- [`docs/reference/config.md`](reference/config.md) — `config!`
- [`docs/reference/wrap.md`](reference/wrap.md) — `#[wrap]`
- [`docs/reference/openapi.md`](reference/openapi.md) — `#[openapi]` / `#[openapi_op]`
- [`docs/reference/delegate.md`](reference/delegate.md) — `#[delegate]`
- [`docs/reference/retry.md`](reference/retry.md) — `#[retry]`
- [`docs/reference/rate-limit.md`](reference/rate-limit.md) — `#[rate_limit]`
- [`docs/reference/circuit-breaker.md`](reference/circuit-breaker.md) — `#[circuit_breaker]`
- [`docs/reference/param-attrs.md`](reference/param-attrs.md) — `#[tool_min]`, `#[tool_pattern]`, `#[param_tool]`, etc.

### Tutorial, cheatsheet, and migration

- [`docs/tutorials/getting-started.md`](tutorials/getting-started.md) — five-chapter walkthrough
- [`docs/quickstart.md`](quickstart.md) — 5-minute tour
- [`docs/wrap-cheatsheet.md`](wrap-cheatsheet.md) — one-page cheat sheet
- [`docs/wrap-architecture.md`](wrap-architecture.md) — long-form wrap-feature reference
- [`docs/migration/v0.4-to-v0.5.md`](migration/v0.4-to-v0.5.md) — upgrade guide

### Architecture, stability, AI integration, cross-language

- [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) — overall crate topology
- [`docs/API_STABILITY.md`](API_STABILITY.md) — semver policy
- [`docs/AI_INTEGRATION.md`](AI_INTEGRATION.md) — Ollama, OpenAI, Claude
- [`docs/MCP_ARCHITECTURE.md`](MCP_ARCHITECTURE.md) — MCP server
- [`docs/CROSS_LANGUAGE.md`](CROSS_LANGUAGE.md) — Python, JS, Go, curl clients

### ADRs (architecture decision records)

- [ADR-0001](adr/0001-async-executor-type-erasure.md) — `AsyncExecutor` uses type erasure, not generics
- [ADR-0002](adr/0002-phf-map-for-openapi-ops.md) — OpenAPI operations use `phf::Map`, not `HashMap`
- [ADR-0003](adr/0003-sync-from-async-via-block-on-dyn.md) — sync-from-async bridge uses `block_on_dyn`
- [ADR-0004](adr/0004-circuit-breaker-v1-observe-only.md) — `#[circuit_breaker]` v1 is observe-only
- [ADR-0005](adr/0005-wrap-reuses-tool-codegen.md) — `#[wrap]` re-uses the `#[tool]` codegen pipeline
- [ADR-0006](adr/0006-openapi-spec-path-resolution.md) — spec path resolution uses `Span::local_file()`

### Source pointers (for when docs are not enough)

- [`tokitai/src/lib.rs`](../../tokitai/src/lib.rs) — top-level umbrella crate
- [`tokitai-core/src/lib.rs`](../../tokitai-core/src/lib.rs) — `ToolDefinition`, `ToolProvider`, `ToolCaller`, `AsyncExecutor`
- [`tokitai-macros/src/lib.rs`](../../tokitai-macros/src/lib.rs) — every proc-macro entry point
- [`tokitai-macros/src/tool/codegen/wrappers.rs`](../../tokitai-macros/src/tool/codegen/wrappers.rs) — the generated `__call_<name>` shape
- [`tokitai-macros/src/tool/wrap_openapi/mod.rs`](../../tokitai-macros/src/tool/wrap_openapi/mod.rs) — OpenAPI spec resolution
- [`examples/`](../../examples/) — runnable examples for every feature

### Where to ask

- [GitHub Issues](https://github.com/silverenternal/tokitai/issues) — bugs and feature requests
- [GitHub Discussions](https://github.com/silverenternal/tokitai/discussions) — questions and design talk
- [docs.rs/tokitai](https://docs.rs/tokitai) — rendered API reference
- [crates.io/crates/tokitai](https://crates.io/crates/tokitai) — published crate
