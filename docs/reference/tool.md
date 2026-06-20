# `#[tool]`

> The primary proc-macro attribute in Tokitai: place it on an `impl` block
> and every `pub` method becomes an AI-callable tool, with a
> compile-time-generated JSON Schema, a `call_tool` dispatcher, and a
> `ToolProvider` / `ToolCaller` trait impl.

## Syntax

```rust,ignore
#[tool]                                       // on the impl block
impl MyType {
    #[tool(name = "...", desc = "...")]      // optionally on a method
    pub async fn my_method(&self, ...) -> ... { ... }
}
```

`#[tool]` is **block-level** (on the `impl`) and **optionally
method-level** (inside the `impl`). The block-level form has no
arguments; the method-level form accepts per-method overrides such as
`name`, `desc`, `tags`, `deprecated`, `validate`, etc.

## Arguments

### Block-level (`#[tool]` on the `impl`)

| Argument | Type | Default | Description |
|---|---|---|---|
| _(none)_ | — | — | The impl-block form takes no arguments in v0.6.0. Per-method configuration is done on individual methods. |

### Method-level (`#[tool(...)]` on a method)

| Argument | Type | Default | Description |
|---|---|---|---|
| `name` | `&str` | method name | Override the tool name shown to the LLM. |
| `desc` / `description` | `&str` | doc comment | Override the tool description. |
| `tags` | `[&str]` | `[]` | Free-form tags; serialized into the JSON Schema's `x-tags` field. |
| `group` | `&str` | _none_ | Logical group for filtering. |
| `visible` | `bool` | `true` | If `false`, the method is hidden from `tool_definitions()` (still callable by name). |
| `skip` | _flag_ | _none_ | Exclude this method from `tool_definitions()` entirely. |
| `deprecated` | `bool` | `false` | Mark the tool as deprecated. |
| `replaced_by` | `&str` | _none_ | Tool name to use instead. |
| `deprecated_note` | `&str` | _none_ | Human-readable deprecation message. |
| `deprecated_since` | `&str` | _none_ | Version / date string. |
| `remove_in` | `&str` | _none_ | Planned removal version. |
| `version` | `&str` | _none_ | Tool version string. |
| `return_description` / `returns` | `&str` | _none_ | Description of the return value. |
| `context` | `&str` | _none_ | Free-form context (e.g. `"async"`). |
| `example_input` / `example` | JSON value | _none_ | Example input object. |
| `example_output` | `&str` | _none_ | Example output (string). |
| `param_order` | `[&str]` | _none_ | Override the parameter ordering in the generated schema. |
| `hidden_params` | `[&str]` | `[]` | Hide specific parameters from the schema. |
| `alias` | `[&str]` | `[]` | Alternate names the dispatcher will recognise. |
| `allow` | `[&str]` | `[]` | Suppress specific warning codes (e.g. `"deprecated_missing_replaced_by"`). |
| `cache` | `&str` | _none_ | Cache hint (e.g. `"30s"`). |
| `rate_limit` | `&str` | _none_ | Inline rate-limit hint (separate from the `#[rate_limit]` decorator). |
| `min_<param>`, `max_<param>` | `f64` | _none_ | Per-parameter numeric bounds. |
| `min_length_<param>`, `max_length_<param>` | `usize` | _none_ | Per-parameter string-length bounds. |
| `min_items_<param>`, `max_items_<param>` | `usize` | _none_ | Per-parameter array-length bounds. |
| `multiple_of_<param>` | `f64` | _none_ | Per-parameter multiple-of constraint. |
| `pattern_<param>` | `&str` | _none_ | Per-parameter regex. |
| `one_of_<param>`, `enum_values_<param>` | list | _none_ | Per-parameter enum. |
| `default_<param>`, `example_<param>` | JSON | _none_ | Per-parameter default / example. |
| `validate_msg_<param>` | `&str` | _none_ | Per-parameter custom failure message. |

## Examples

### Minimal

```rust,ignore
use tokitai::tool;

pub struct Greeter;

#[tool]
impl Greeter {
    /// Say hello to `name`.
    pub fn hello(&self, name: String) -> String {
        format!("Hello, {name}!")
    }
}

fn main() {
    let g = Greeter;
    let v = g.call_tool("hello", &serde_json::json!({"name": "world"})).unwrap();
    assert_eq!(v, serde_json::json!("Hello, world!"));
}
```

### Common usage

```rust,ignore
use tokitai::tool;

pub struct Calculator;

#[tool]
impl Calculator {
    /// Add two numbers.
    #[tool(
        name = "add_numbers",
        desc = "Add two integers and return their sum.",
        tags = ["math", "arithmetic"],
        min_a = 0,
        max_a = 1_000_000,
        min_b = 0,
        max_b = 1_000_000,
        example_a = 10,
        example_b = 20,
    )]
    pub async fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Multiply, deprecated in favour of `add_numbers` repeated.
    #[tool(
        deprecated,
        replaced_by = "add_numbers",
        deprecated_note = "LLMs multiply by adding N times anyway.",
        deprecated_since = "0.5.0",
        remove_in = "0.7.0",
    )]
    pub async fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }

    // Not public → not a tool.
    fn helper(&self) -> i32 { 0 }
}

fn main() {
    let c = Calculator;
    let defs = Calculator::tool_definitions();
    assert_eq!(defs.len(), 2); // helper is not public

    let v = c.call_tool("add_numbers", &serde_json::json!({"a": 2, "b": 3})).await.unwrap();
    assert_eq!(v, serde_json::json!(5));
}
```

### Edge case

`#[tool]` on a `struct` (no `impl`) is accepted as a no-op marker so
existing scaffolding that uses `#[tool]` on both a struct and its impl
keeps compiling:

```rust,ignore
use tokitai::tool;

#[tool]      // no-op: marks the type as a "tool type"
pub struct Calculator;

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 { a + b }
}
```

Generics are rejected: a method declared with type parameters emits
`compile_error!(...)` for the per-method `__TOOL_DEF_*` constant but
the rest of the impl block still compiles.

## Generated code

For an impl block containing a single `pub fn add(&self, a: i32, b: i32) -> i32`,
the macro emits (in addition to your original code) five new items:

1. **A compile-time tool-definition function** for every public method.
   Stored behind a `LazyLock` so `ToolDefinition::new` is called once:

   ```rust,ignore
   fn __TOOL_DEF_ADD() -> &'static ::tokitai::ToolDefinition {
       static DEF: ::std::sync::LazyLock<::tokitai::ToolDefinition> =
           ::std::sync::LazyLock::new(|| {
               ::tokitai::ToolDefinition::new(
                   "add",
                   "Add two numbers.",
                   r#"{"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}},"required":["a","b"]}"#,
               )
           });
       &*DEF
   }
   ```

2. **A compile-time tool counter**:

   ```rust,ignore
   #[allow(dead_code)]
   const __TOOL_COUNT: usize = 1;
   ```

3. **A `__get_tool_definitions` function** that collects every
   `__TOOL_DEF_*` into a `LazyLock<Vec<ToolDefinition>>` and applies
   any registered `config!` overrides:

   ```rust,ignore
   fn __get_tool_definitions() -> &'static [::tokitai_core::ToolDefinition] {
       static TOOLS: ::std::sync::LazyLock<::std::vec::Vec<::tokitai_core::ToolDefinition>> =
           ::std::sync::LazyLock::new(|| {
               let mut defs = ::std::vec::Vec::from([
                   <Calculator>::__TOOL_DEF_ADD().clone(),
               ]);
               for def in &mut defs {
                   let configs = ::tokitai_core::GLOBAL_CONFIG_REGISTRY.get(&def.name);
                   if !configs.is_empty() {
                       def.apply_configs(&configs);
                   }
               }
               defs
           });
       &TOOLS
   }
   ```

4. **A `call_tool` dispatcher** (sync or async depending on whether
   any method is `async fn`):

   ```rust,ignore
   pub async fn call_tool(
       &self,
       name: &str,
       args: &serde_json::Value,
   ) -> Result<serde_json::Value, ::tokitai::ToolError> {
       match name {
           "add" => self.__call_add(args).await,
           _ => Err(::tokitai::ToolError::not_found("unknown tool")),
       }
   }
   ```

5. **A per-method `__call_<name>` wrapper** that parses the JSON args
   into your parameter types, runs validation, then invokes your
   method:

   ```rust,ignore
   async fn __call_add(
       &self,
       args: &serde_json::Value,
   ) -> Result<serde_json::Value, ::tokitai::ToolError> {
       let a = args.get("a").ok_or_else(||
           ::tokitai::ToolError::validation_error("missing required parameter 'a' (type: i32)"))?;
       let mut a: i32 = serde_json::from_value(a.clone())
           .map_err(|e| ::tokitai::ToolError::validation_error("parameter type mismatch: 'a' (expected type: i32)"))?;
       // ... same for b ...
       Ok(serde_json::to_value(self.add(a, b).await).unwrap())
   }
   ```

   For mixed (sync + async) impl blocks the macro additionally emits
   a `__call_<name>_sync` for every sync method, and a `call_tool_sync`
   that points at them. This is so the always-sync `ToolCaller` trait
   impl can route uniformly.

Outside the impl block the macro emits two trait impls:

```rust,ignore
impl ::tokitai_core::ToolProvider for Calculator {
    fn tool_definitions() -> &'static [::tokitai_core::ToolDefinition] {
        Self::__get_tool_definitions()
    }
    fn tool_count() -> usize { Self::__TOOL_COUNT }
}

impl ::tokitai_core::ToolCaller for Calculator {
    fn call_tool(&self, name: &str, args: &::tokitai_core::serde_types::Value)
        -> Result<::tokitai_core::serde_types::Value, ::tokitai_core::ToolError>
    {
        Self::call_tool_sync(self, name, args)
    }
}
```

Source: [`tokitai-macros/src/tool/mod.rs`](../../tokitai-macros/src/tool/mod.rs)
and [`tokitai-macros/src/tool/codegen/*`](../../tokitai-macros/src/tool/codegen/).

## Interactions

- **With `#[tool_type]`**: see [`tool-type.md`](tool-type.md). Use
  `#[tool_type]` on a struct to give it a hand-written schema; the
  struct is then referenced from a `#[tool]` method.
- **With `config!`**: the `__get_tool_definitions` function consults
  `tokitai_core::GLOBAL_CONFIG_REGISTRY` at first access and applies
  any registered overrides via `ToolDefinition::apply_configs`.
  See [`config.md`](config.md).
- **With `#[wrap]`**: `#[wrap]` reuses 100% of this codegen pipeline.
  See [`wrap.md`](wrap.md).
- **With `#[openapi]`**: same pipeline, plus a `phf::Map` of
  operations. See [`openapi.md`](openapi.md).
- **With `#[delegate]`**: `#[delegate]` emits only the
  `__TOOL_DEF_*` / `__call_*` artifacts, no dispatcher. See
  [`delegate.md`](delegate.md).
- **With `#[retry]` / `#[rate_limit]` / `#[circuit_breaker]`**:
  stacking order is outer-attribute-last. See
  [`retry.md`](retry.md), [`rate-limit.md`](rate-limit.md),
  [`circuit-breaker.md`](circuit-breaker.md), and the wrap-architecture
  doc's [Composition rules](../wrap-architecture.md#5-composition-rules).

## Errors

The macro can produce these `compile_error!` messages and stderr
warnings:

| Code | Severity | Triggered by | Message |
|---|---|---|---|
| _compile error_ | hard | a `#[tool]` method has generic type parameters | `"tool method \`<name>\` uses unsupported generic parameters"` |
| `W001` | warning | a `#[tool(deprecated)]` method has no `replaced_by` | `"[tokitai] [W001] deprecated method `<name>` missing `replaced_by`"` |
| `W002` | warning | an `Option<T>` parameter has no `default_*` or `example_*` | `"[tokitai] [W002] optional param `<name>` lacks default/example"` |
| `W003` | warning | `context = "async"` on a sync method | `"[tokitai] [W003] method `<name>` has `context=\"async\"` but is not async"` |

W001–W003 can be silenced per-method with
`#[tool(allow = ["deprecated_missing_replaced_by", "option_no_default", "context_async_mismatch"])]`,
or globally with the `TOKITAI_QUIET=1` env var, or made visible under
`cfg(test)` with the `TOKITAI_SHOW_WARNINGS=1` env var.

Source: [`tokitai-macros/src/tool/mod.rs`](../../tokitai-macros/src/tool/mod.rs)
(`should_show_warnings` and the `for tool in &tool_methods` loop).

## See also

- Tutorial: [`docs/USAGE.md`](../USAGE.md) — the long-form walk
  through `#[tool]`.
- Quickstart: [`docs/quickstart.md`](../quickstart.md).
- Architecture: [`docs/wrap-architecture.md`](../wrap-architecture.md)
  for how `#[tool]` fits with the wrap features.
- Rustdoc:
  [`tokitai-macros/src/lib.rs`](../../tokitai-macros/src/lib.rs)
  (`pub fn tool`).
- Example: [`examples/basic_usage.rs`](../../examples/basic_usage.rs).
- Example: [`examples/multi_tool_chat.rs`](../../examples/multi_tool_chat.rs)
  (multiple `#[tool]` impls in one binary).
- Example: [`examples/ollama_integration.rs`](../../examples/ollama_integration.rs)
  (a real LLM consumer of the generated definitions).
