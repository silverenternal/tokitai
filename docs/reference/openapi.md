# `#[openapi]` and `#[openapi_op]`

> Drive a `ToolProvider` from an OpenAPI 3 spec file. Place `#[openapi]`
> on the `impl` block to point at the spec, then mark each method with
> `#[openapi_op(operation_id = "...")]` to bind it to a specific
> `operationId` from the spec. The macro reads the spec at proc-macro
> compile time and bakes a `phf::Map` of operations into the binary.

## Syntax

```rust,ignore
#[openapi(spec = "spec.json", base_url = "https://...", target = MyType)]
impl MyClient {
    #[openapi_op(operation_id = "createChatCompletion")]
    pub async fn create_chat_completion(&self, body: ChatRequest)
        -> Result<ChatResponse, reqwest::Error> { /* body */ }
}
```

`#[openapi]` is **block-level**. `#[openapi_op]` is **method-level**
and only meaningful inside an `#[openapi]`-annotated `impl` block.

## Arguments

### `#[openapi(...)]` (block-level)

| Argument | Type | Default | Description |
|---|---|---|---|
| `spec` | `&str` (file path) | _required_ | Path to the OpenAPI 3 spec (JSON; YAML must be pre-converted). Relative paths are resolved against the source file containing the attribute. |
| `base_url` | `&str` | _none_ | Optional base URL prefix; stored on the impl for downstream use. |
| `target` | `Ident` | _none_ | Optional type override for the static name. |

### `#[openapi_op(...)]` (method-level)

| Argument | Type | Default | Description |
|---|---|---|---|
| `operation_id` | `&str` | _required_ | The `operationId` from the spec to bind this method to. |

## Examples

### Minimal

```rust,ignore
use tokitai::{openapi, openapi_op};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PingRequest { pub message: String }

#[derive(Serialize, Deserialize)]
pub struct PingResponse { pub ok: bool }

#[openapi(spec = "ping.json")]
impl PingClient {
    #[openapi_op(operation_id = "ping")]
    pub async fn ping(&self, body: PingRequest) -> Result<PingResponse, String> {
        Ok(PingResponse { ok: true })
    }
}
```

### Common usage

```rust,ignore
use tokitai::{openapi, openapi_op, ToolProvider};

#[openapi(
    spec = "openai_chat.json",
    base_url = "https://api.openai.com/v1",
)]
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

    #[openapi_op(operation_id = "listModels")]
    pub async fn list_models(&self) -> Result<Vec<String>, reqwest::Error> {
        Ok(vec!["gpt-4o".to_string()])
    }
}

fn main() {
    let defs = OpenAIClient::tool_definitions();
    assert!(defs.iter().any(|d| d.name == "createChatCompletion"));
    assert!(defs.iter().any(|d| d.name == "listModels"));
}
```

### Edge case

If a method on the impl block has **no** `#[openapi_op]` attribute, it
is silently skipped — the macro only emits tool definitions for
methods that opt in. If `operation_id` is given but does not exist in
the spec, the macro emits a `compile_error!` naming the missing ID.

```rust,ignore
use tokitai::{openapi, openapi_op};

#[openapi(spec = "spec.json")]
impl Mixed {
    #[openapi_op(operation_id = "foo")]
    pub async fn foo(&self) -> Result<(), String> { Ok(()) }

    // No `#[openapi_op]` -> not a tool, even though it's `pub`.
    pub async fn helper(&self) -> u32 { 0 }
}
```

## Generated code

For each `#[openapi_op]`-marked method, the macro emits the same
artifacts as `#[tool]` (`__TOOL_DEF_*`, `__call_*`, `call_tool`,
`call_tool_sync`, `__TOOL_COUNT`, `__get_tool_definitions`, the
`configure_tool` no-op stub, the `ToolProvider` / `ToolCaller` impls).
The differences from `#[tool]` are:

1. The `__TOOL_DEF_*` function's `description` is taken from the spec
   (`summary` or `description`) rather than from a doc comment.
2. The `tool_name` is the `operationId` rather than the Rust method
   name.
3. The `configure_tool` method is a no-op (OpenAPI metadata is
   fixed at compile time):

   ```rust,ignore
   /// Compile-time-only stub. OpenAPI-derived metadata cannot be
   /// overridden at runtime; this method exists for trait-shape
   /// parity with `#[tool]`.
   pub fn configure_tool(_tool_name: &str, _configs: &[::tokitai_core::ToolConfig]) {}
   ```

4. **Two extra statics are emitted** outside the impl block:

   ```rust,ignore
   pub static __OPENAPI_OPS_OpenAIClient: ::phf::Map<
       &'static str,
       __OpenApiOp_OpenAIClient,
   > = /* phf::Map built at compile time, keyed by operationId */;

   pub static __OPENAPI_SPEC_RAW: &'static str =
       include_str!("<absolute spec path>");
   ```

   The `__OpenApiOp_<Type>` struct is per-impl and holds the path,
   HTTP method, and original `operationId` so consumers can
   introspect the spec at runtime.

5. The `__get_tool_definitions` function does **not** apply
   `GLOBAL_CONFIG_REGISTRY` entries (because the metadata is fixed):

   ```rust,ignore
   fn __get_tool_definitions() -> &'static [::tokitai_core::ToolDefinition] {
       static TOOLS: ::std::sync::LazyLock<::std::vec::Vec<::tokitai_core::ToolDefinition>> =
           ::std::sync::LazyLock::new(|| {
               ::std::vec::Vec::from([
                   <OpenAIClient>::__TOOL_DEF_CREATE_CHAT_COMPLETION().clone(),
                   <OpenAIClient>::__TOOL_DEF_LIST_MODELS().clone(),
               ])
           });
       &TOOLS
   }
   ```

Source:
[`tokitai-macros/src/tool/wrap_openapi/codegen.rs`](../../tokitai-macros/src/tool/wrap_openapi/codegen.rs)
(`pub fn expand_impl`).

## Interactions

- **With `#[retry]` / `#[rate_limit]` / `#[circuit_breaker]`**:
  compose normally — the resilience decorators wrap the user-written
  method body the same way they do under `#[tool]`. See
  [`retry.md`](retry.md), [`rate-limit.md`](rate-limit.md),
  [`circuit-breaker.md`](circuit-breaker.md).
- **With `config!`**: silently ignored — `__get_tool_definitions` for
  `#[openapi]` blocks does not apply registry entries. See
  [`config.md`](config.md).
- **With `#[wrap]`**: do not combine on the same impl block. Each
  attribute is a full block-level wrapper and they will collide on
  the generated `__TOOL_DEF_*` / `call_tool` items.
- **Re-export**: `#[openapi]` and `#[openapi_op]` are re-exported from
  `tokitai` as `tokitai::openapi` / `tokitai::openapi_op`.

## Errors

| Trigger | Message |
|---|---|
| `spec` argument missing | "`#[openapi]` requires `spec = "..."`" |
| Spec file cannot be read | "`#[openapi]` could not read spec file `<path>`: `<io error>`" |
| Spec file is not valid OpenAPI 3 JSON | "`#[openapi]` could not parse spec `<path>` as OpenAPI 3 JSON: <err>" |
| Unknown `#[openapi]` key | "`unknown `#[openapi]` argument `<key>` (expected: spec, base_url, target)`" |
| `#[openapi_op]` without `operation_id` | "`#[openapi_op]` is missing `operation_id = "..."`" |
| `operation_id` not found in spec | "`operation_id `<id>` not found in OpenAPI spec `<path>`" |
| Unknown `#[openapi_op]` key | "`unknown `#[openapi_op]` argument `<key>` (expected: operation_id)`" |

Source:
[`tokitai-macros/src/tool/wrap_openapi/mod.rs`](../../tokitai-macros/src/tool/wrap_openapi/mod.rs)
and
[`tokitai-macros/src/tool/wrap_openapi/extract.rs`](../../tokitai-macros/src/tool/wrap_openapi/extract.rs).

## See also

- Tutorial: [`docs/USAGE.md`](../USAGE.md) (`OpenAPI-driven wrappers`
  section).
- Architecture: [`docs/wrap-architecture.md`](../wrap-architecture.md)
  (§4.2 — full deep-dive).
- Cheatsheet: [`docs/wrap-cheatsheet.md`](../wrap-cheatsheet.md).
- Rustdoc:
  [`tokitai-macros/src/lib.rs`](../../tokitai-macros/src/lib.rs)
  (`pub fn openapi`, `pub fn openapi_op`).
- Example: [`examples/wrap_openapi.rs`](../../examples/wrap_openapi.rs)
  (full source with a fake `openai_chat.json`).
- **tracking-issue:** [#36](https://github.com/silverenternal/tokitai/issues/36) (attribute not yet exported in 0.5.x).
- Example: [`examples/mcp_http_server.rs`](../../examples/mcp_http_server.rs)
  serves `#[openapi]`-generated tools over HTTP.
