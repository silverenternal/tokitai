# `#[delegate]`

> Per-method transparent forwarding: write a method **signature only**
> (no body) and the macro injects a body that calls
> `<to>.<method_name>(<args>)` (with `.await` for `async fn`). The
> matching `__TOOL_DEF_*` and `__call_*` items are emitted so the
> method is wireable into a `call_tool` dispatcher.

## Syntax

```rust,ignore
#[delegate(to = "self.inner")]
pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, Error>;
```

`#[delegate]` is **method-level** and only meaningful on a method
**signature** (a `pub fn ...;` declaration, no body, terminated with
`;`).

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `to` | `&str` (a Rust expression) | _required_ | The expression that the call is forwarded to. For instance methods, the generated body is `<to>.<method_name>(<args>)`. For associated functions, `<to>` is the entire body verbatim. |

The `to` string is parsed as a `syn::Expr` so it accepts any Rust
expression that type-checks in the method's context (`self.inner`,
`Config::default()`, a path, etc.).

## Examples

### Minimal

```rust,ignore
use tokitai::delegate;

pub struct Inner;
impl Inner {
    pub fn ping(&self) -> bool { true }
}

pub struct Wrapper {
    pub inner: Inner,
}

impl Wrapper {
    #[delegate(to = "self.inner")]
    pub fn ping(&self) -> bool;
}
```

### Common usage

```rust,ignore
use serde::Serialize;
use tokitai::delegate;

#[derive(Default)]
pub struct InnerClient { pub counter: std::cell::Cell<u32> }

impl InnerClient {
    pub fn ping(&self) -> bool {
        self.counter.set(self.counter.get() + 1);
        true
    }

    pub fn get_email(&self, uid: u64) -> String {
        format!("user-{}@example.com", uid)
    }
}

#[derive(Serialize)]
pub struct Config { pub name: &'static str }

impl Config {
    pub fn default() -> Self { Config { name: "default" } }
    pub fn default_config(&self) -> Self { Config { name: "default" } }
}

pub struct OpenAIClient {
    pub inner: InnerClient,
    pub db: InnerClient,
}

impl OpenAIClient {
    #[delegate(to = "self.inner")]
    pub fn ping(&self) -> bool;

    #[delegate(to = "self.db")]
    pub fn get_email(&self, uid: u64) -> String;

    // Associated function: `to` is the entire body, no `.method_name(...)` appended.
    #[delegate(to = "Config::default()")]
    pub fn default_config() -> Config;
}
```

### Edge case

`#[delegate]` deliberately does **not** emit a `call_tool` dispatcher
or a `ToolProvider` impl. That is what `#[tool]` would do, and emitting
both would clash. Users wire the dispatcher by hand:

```rust,ignore
use tokitai::delegate;
use tokitai::ToolDefinition;

impl OpenAIClient {
    #[delegate(to = "self.inner")]
    pub fn ping(&self) -> bool;
}

fn collect_definitions(c: &OpenAIClient) -> Vec<ToolDefinition> {
    vec![OpenAIClient::__TOOL_DEF_PING().clone()]
}
```

For instance methods, the macro also emits a `__call_ping(args)`
wrapper that does JSON parsing + validation, so you can dispatch by
name from a JSON payload.

## Generated code

For an instance method like
`#[delegate(to = "self.inner")] pub async fn chat(&self, req: ChatRequest) -> Result<...>;`,
the macro emits three groups of items:

1. **The forwarded method itself**, with the same signature and a
   generated body that calls `<to>.<method_name>(<args>).await`:

   ```rust,ignore
   #[doc = "Description from `///` doc comments"]
   pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, OpenAIError> {
       self.inner.chat(req).await
   }
   ```

2. **A `__TOOL_DEF_<NAME>` function** with the same shape as
   `#[tool]` emits:

   ```rust,ignore
   fn __TOOL_DEF_CHAT() -> &'static ::tokitai::ToolDefinition {
       static DEF: ::std::sync::LazyLock<::tokitai::ToolDefinition> =
           ::std::sync::LazyLock::new(|| {
               ::tokitai::ToolDefinition::new(
                   "chat",
                   "Description from `///` doc comments",
                   r#"{"type":"object","properties":{"req":{…}},"required":["req"]}"#,
               )
           });
       &*DEF
   }
   ```

3. **A `__call_<NAME>` (and `_sync` for `async`) wrapper** with the
   same shape as `#[tool]` emits:

   ```rust,ignore
   async fn __call_chat(
       &self,
       args: &serde_json::Value,
   ) -> Result<serde_json::Value, ::tokitai::ToolError> {
       // JSON-arg parsing, validation, then: self.chat(req).await
   }
   ```

For **associated functions** (no `&self`), the generated body is
`<to>` verbatim — appending `.<method_name>(<args>)` would recurse into
the method being defined. `__TOOL_DEF_*` and `__call_*` items are
**not** emitted for associated functions because they reference
`self`.

Source:
[`tokitai-macros/src/tool/delegate/codegen.rs`](../../tokitai-macros/src/tool/delegate/codegen.rs)
(`pub fn generate` and `fn build_forwarded_method`).

## Interactions

- **With `#[tool]`**: do not stack on the same method — `#[tool]`
  expects a method body, `#[delegate]` requires no body. You can
  however put `#[delegate]` methods inside a `#[tool]` impl block
  and combine them with hand-written methods; the `#[tool]` block's
  dispatcher will see all of them uniformly.
- **With `#[wrap]`**: `methods = [...]` lists which `#[delegate]`
  methods become tools. See [`wrap.md`](wrap.md).
- **With `#[openapi]`**: not meaningful — `#[openapi_op]` already
  binds a method to a spec operation; there is no forwarding to do.
- **Generic methods**: not supported. The macro emits
  `compile_error!("#[delegate] does not support generic methods (use a concrete type)")`
  if the signature has type parameters.

## Errors

| Trigger | Message |
|---|---|
| Method has a body | `"#[delegate] is meant to be applied to a method signature (no body); remove the existing method body"` |
| Method is generic | `"#[delegate] does not support generic methods (use a concrete type)"` |
| `to = "..."` is not a valid `syn::Expr` | `"#[delegate]: failed to parse `to = "..."` as a Rust expression: <syn err>"` |
| `#[delegate]` not used on `pub` fn | (No body is required, but the user should still write `pub`; the macro re-adds `pub` if it was missing.) |
| First `#[delegate]` key is not `to` | `"expected `to` in #[delegate(to = "...")]"` |

Source:
[`tokitai-macros/src/tool/delegate/mod.rs`](../../tokitai-macros/src/tool/delegate/mod.rs)
and
[`tokitai-macros/src/tool/delegate/extract.rs`](../../tokitai-macros/src/tool/delegate/extract.rs).

## See also

- Tutorial: [`docs/USAGE.md`](../USAGE.md) (`#[delegate]` section).
- Architecture: [`docs/wrap-architecture.md`](../wrap-architecture.md)
  (§4.3 — full deep-dive).
- Cheatsheet: [`docs/wrap-cheatsheet.md`](../wrap-cheatsheet.md).
- Rustdoc:
  [`tokitai-macros/src/lib.rs`](../../tokitai-macros/src/lib.rs)
  (`pub fn delegate`).
- Example: [`examples/wrap_demo.rs`](../../examples/wrap_demo.rs) —
  the curated forwarding pattern `#[delegate]` is designed to
  automate (since `#[delegate]` is not yet exported in 0.5.x).
- **tracking-issue:** [#32](https://github.com/silverenternal/tokitai/issues/32)
