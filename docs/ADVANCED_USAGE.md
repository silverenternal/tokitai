# Tokitai Advanced Usage Guide

**Version**: 0.5.0 | **Last updated**: 2026-06-02

This guide covers Tokitai's advanced features and best practices.

## Table of Contents

- [`#[tool(skip)]` excluding methods](#toolskip-excluding-methods)
- [Sync and async tools](#sync-and-async-tools)
- [Custom error types](#custom-error-types)
- [Support for complex types](#support-for-complex-types)
- [Composing multiple tools](#composing-multiple-tools)
- [Handling `call_tool` return values](#handling-call_tool-return-values)
- [Performance optimization tips](#performance-optimization-tips)

---

## `#[tool(skip)]` excluding methods

By default, every `pub` method in a `#[tool]` impl block is exposed to the AI. To exclude internal helpers or debug-only methods, annotate them with `#[tool(skip)]`.

### Example

```rust
use tokitai::tool;

pub struct DataProcessor {
    cache: std::collections::HashMap<String, String>,
}

#[tool]
impl DataProcessor {
    /// Process input and return the result
    pub fn process(&self, input: String) -> String {
        let cached = self.get_cached(&input);
        if let Some(result) = cached {
            return result;
        }
        // Processing logic...
        format!("Processed: {}", input)
    }

    /// Internal cache lookup - not exposed to the AI
    #[tool(skip)]
    pub fn get_cached(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    /// Debug helper - not exposed to the AI
    #[tool(skip)]
    pub fn debug_info(&self) -> String {
        format!("Cache size: {}", self.cache.len())
    }
}
```

In this example:

- `process` is exposed to the AI
- `get_cached` and `debug_info` are not exposed

---

## Sync and async tools

Tokitai supports both synchronous and asynchronous tool methods. The macro generates the right `call_tool` variant for each.

### Synchronous tools

```rust
use tokitai::tool;

pub struct Calculator;

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// Synchronous call
let calc = Calculator;
let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20}))?;
```

### Asynchronous tools

```rust
use tokitai::tool;

pub struct DatabaseService;

#[tool]
impl DatabaseService {
    pub async fn query(&self, sql: String) -> Result<Vec<serde_json::Value>, String> {
        // Async database query
        // tokio_postgres::query(...)
        Ok(vec![])
    }
}

// Asynchronous call
let db = DatabaseService;
let result = db.call_tool("query", &serde_json::json!({"sql": "SELECT * FROM users"})).await?;
```

### Mixed tools (sync and async in the same block)

When an impl block contains both sync and async methods, the macro generates:

- `call_tool()` - the async version
- `call_tool_sync()` - the synchronous, blocking version (uses `tokio::runtime::Handle::block_on` internally)

```rust
use tokitai::tool;

pub struct HybridService;

#[tool]
impl HybridService {
    // Sync method
    pub fn compute(&self, data: Vec<i32>) -> i32 {
        data.iter().sum()
    }

    // Async method
    pub async fn fetch(&self, url: String) -> Result<String, String> {
        // reqwest::get(&url).await?.text().await
        Ok("data".to_string())
    }
}

// In an async context
let service = HybridService;

// Async call (preferred)
let result = service.call_tool("compute", &serde_json::json!({"data": [1, 2, 3]})).await?;

// Sync call (blocks the current thread)
let result = service.call_tool_sync("compute", &serde_json::json!({"data": [1, 2, 3]}))?;
```

---

## Custom error types

Tokitai supports custom error types returned by tool methods. The macro handles the conversion to `ToolError` automatically.

### Using `thiserror`

```rust
use tokitai::tool;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CalculatorError {
    #[error("divisor cannot be zero")]
    DivisionByZero,
    #[error("overflow: {0}")]
    Overflow(String),
}

pub struct Calculator;

#[tool]
impl Calculator {
    pub fn divide(&self, a: f64, b: f64) -> Result<f64, CalculatorError> {
        if b == 0.0 {
            Err(CalculatorError::DivisionByZero)
        } else {
            Ok(a / b)
        }
    }
}

// The error is converted to tokitai::ToolError at the call site
let calc = Calculator;
match calc.call_tool("divide", &serde_json::json!({"a": 10.0, "b": 0.0})) {
    Ok(_) => println!("success"),
    Err(e) => println!("error: {:?}", e), // ToolError::InternalError
}
```

### Error-handling best practices

1. **Use `Result` as the return type** so the macro converts errors for you
2. **Provide meaningful error messages** that help the caller understand what went wrong
3. **Avoid leaking internal details** - messages should be user-friendly

---

## Support for complex types

### `Option` parameters

Parameters of type `Option<T>` are optional. If the AI omits them, the value is `None`.

```rust
use tokitai::tool;

pub struct Greeter;

#[tool]
impl Greeter {
    /// Greet someone; `language` is optional
    pub fn greet(&self, name: String, language: Option<String>) -> String {
        match language.as_deref() {
            Some("zh") => format!("Ni hao, {}!", name),
            Some("es") => format!("¡Hola, {}!", name),
            _ => format!("Hello, {}!", name),
        }
    }
}

// Without the optional parameter
let result = greeter.call_tool("greet", &serde_json::json!({"name": "Alice"}))?;
// Output: Hello, Alice!

// With the optional parameter
let result = greeter.call_tool("greet", &serde_json::json!({"name": "Bob", "language": "zh"}))?;
// Output: Ni hao, Bob!
```

### `Vec` parameters

```rust
use tokitai::tool;

pub struct MathService;

#[tool]
impl MathService {
    /// Sum a list of numbers
    pub fn sum(&self, numbers: Vec<i32>) -> i32 {
        numbers.iter().sum()
    }

    /// Filter even numbers
    pub fn filter_even(&self, numbers: Vec<i32>) -> Vec<i32> {
        numbers.into_iter().filter(|n| n % 2 == 0).collect()
    }
}
```

### Custom struct parameters

For complex custom structs, the recommended approach is to take `serde_json::Value` as the parameter type and parse it inside the method:

```rust
use tokitai::tool;
use serde_json::Value;

pub struct UserService;

#[derive(serde::Deserialize)]
struct CreateUserRequest {
    name: String,
    email: String,
    age: Option<i32>,
}

#[tool]
impl UserService {
    /// Create a new user
    pub fn create_user(&self, request: Value) -> Result<Value, String> {
        let req: CreateUserRequest = serde_json::from_value(request)
            .map_err(|e| format!("Parameter parse error: {}", e))?;

        // Creation logic...

        Ok(serde_json::json!({
            "id": 123,
            "name": req.name,
            "email": req.email
        }))
    }
}
```

---

## Composing multiple tools

In larger applications, you may want to combine several tool providers into one.

### Example: a personal-assistant system

```rust
use tokitai::{tool, ToolProvider};
use serde_json::Value;

// Todo management
pub struct TodoManager;

#[tool]
impl TodoManager {
    pub fn add_todo(&self, title: String) -> String {
        format!("Added todo: {}", title)
    }

    pub fn list_todos(&self) -> Value {
        serde_json::json!([])
    }
}

// Note management
pub struct NoteManager;

#[tool]
impl NoteManager {
    pub fn create_note(&self, content: String) -> String {
        "Note created".to_string()
    }

    pub fn list_notes(&self) -> Value {
        serde_json::json!([])
    }
}

// Compose the two providers
pub struct PersonalAssistant {
    todo_manager: TodoManager,
    note_manager: NoteManager,
}

impl PersonalAssistant {
    pub fn new() -> Self {
        Self {
            todo_manager: TodoManager,
            note_manager: NoteManager,
        }
    }

    /// Get all tool definitions (merging multiple providers)
    pub fn get_all_tools(&self) -> Vec<tokitai::ToolDefinition> {
        let mut tools = Vec::new();
        tools.extend_from_slice(TodoManager::tool_definitions());
        tools.extend_from_slice(NoteManager::tool_definitions());
        tools
    }

    /// Unified tool-call entry point
    pub fn call_tool(&self, name: &str, args: &Value) -> Result<Value, String> {
        // Route to the right provider
        match name {
            "add_todo" | "list_todos" => {
                self.todo_manager.call_tool(name, args)
                    .map_err(|e| e.to_string())
            }
            "create_note" | "list_notes" => {
                self.note_manager.call_tool(name, args)
                    .map_err(|e| e.to_string())
            }
            _ => Err(format!("unknown tool: {}", name)),
        }
    }
}
```

---

## Handling `call_tool` return values

`call_tool` returns `Result<serde_json::Value, ToolError>`. A few common ways to handle the value:

### Extract directly

```rust
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}))?;
let sum = result.as_i64().unwrap();
println!("Result: {}", sum);
```

### Deserialize into a concrete type

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct WeatherResponse {
    temperature: f64,
    condition: String,
}

let result = weather.call_tool("get_weather", &json!({"city": "Beijing"}))?;
let weather: WeatherResponse = serde_json::from_value(result)?;
println!("Temperature: {}°C", weather.temperature);
```

### Handle errors

```rust
match calculator.call_tool("divide", &json!({"a": 10, "b": 0})) {
    Ok(result) => println!("Result: {}", result),
    Err(tokitai::ToolError { kind: tokitai::ToolErrorKind::ValidationError, message }) => {
        eprintln!("Parameter validation error: {}", message);
    }
    Err(tokitai::ToolError { kind: tokitai::ToolErrorKind::NotFound, message }) => {
        eprintln!("Tool not found: {}", message);
    }
    Err(e) => {
        eprintln!("Internal error: {:?}", e);
    }
}
```

---

## Performance optimization tips

### 1. Avoid unnecessary temporaries in tool calls

```rust
// Avoid: allocates a new Vec on every call
pub fn process(&self, data: Vec<i32>) -> i32 {
    data.iter().sum()
}

// Prefer: take a slice
pub fn process(&self, data: &[i32]) -> i32 {
    data.iter().sum()
}
```

### 2. For CPU-bound work, consider `spawn_blocking`

When you call a sync tool from async code and the tool runs for a while:

```rust
// A long-running sync tool can block the async event loop.
// Consider using tokio::task::spawn_blocking inside the tool.
pub async fn heavy_computation(&self, n: i32) -> i32 {
    tokio::task::spawn_blocking(move || {
        // CPU-bound logic
        (0..n).sum()
    })
    .await
    .unwrap()
}
```

### 3. Cache tool definitions

Tool definitions are generated at compile time; you do not need to rebuild them on every call:

```rust
// Prefer: call the method directly
let tools = Calculator::tool_definitions();

// Avoid: cloning when you do not need to
let tools = Calculator::tool_definitions().to_vec();
```

---

## Known limitations

1. **Generic methods are not supported** - tool methods cannot be generic
2. **Associated-type restrictions** - return types must be concrete or `Result<T, E>`
3. **Limited `no_std` support** - the full feature set requires `serde` and `serde_json`

---

## Description priority table (frozen, T-002)

Tokitai has **four** sources that can supply a tool's description. They
are merged at compile time and the runtime `tokitai!` config may only
override some of them. The single source of truth is the `const fn`
[`tokitai_core::config::CONFIG_PRIORITY_ORDER`](https://docs.rs/tokitai-core/latest/tokitai_core/config/constant.CONFIG_PRIORITY_ORDER.html)
and its `config_priority_table_md()` renderer.

| Priority | Source | Layer label | Runtime overridable? |
|---------:|--------|-------------|----------------------|
| 1 (highest) | compile-time attribute | `#[tool(desc = "...")]` | No — the runtime registry skips `ToolConfig::Desc` when `ToolDefinition::description_explicit == true`. |
| 2 | compile-time doc | `///` doc comment above the method | Yes — the runtime `tokitai!` block wins over a doc comment. |
| 3 | runtime registry | `tokitai!` config block | n/a (it *is* the runtime layer). |
| 4 (lowest) | synthesized default | `"调用 <method> 方法"` (or similar) | Yes — anything beats the synthesized default. |

**Rendered table (matches the `const` source above):**

1. `#[tool(desc = "...")]` (compile-time, attribute-supplied) — **wins** on conflict
2. doc comment (`///` lines above the method) — used if no `#[tool(desc)]` is present
3. tokitai! config block (`GLOBAL_CONFIG_REGISTRY`) — does **not** override an explicit `#[tool(desc)]`
4. synthesized default (e.g. `"调用 <method> 方法"`) — last-resort fallback

Per-parameter rules:

- `#[param_tool(desc = "...")]` (compile-time) > runtime `tokitai!`
  per-parameter `desc:`. Per-parameter descriptions are independent of
  the tool-level priority table.

This table is enforced by `tokitai/tests/config_override_test.rs` and
unit-tested by `tokitai-core/src/config.rs::tests`. If you change a
priority, update both.

---

## Troubleshooting

### Compile error: `call_tool` is not a future

If all of your tools are synchronous, `call_tool` returns a `Result` rather than a `Future`:

```rust
// Does not compile: awaiting a sync call
let result = calc.call_tool("add", &args).await?;

// Compiles: just call it directly
let result = calc.call_tool("add", &args)?;
```

### Compile error: type inference failure

For complex argument types, an explicit annotation may be needed:

```rust
// Might fail to infer
let result = service.call_tool(name, &args)?;

// Add a type annotation
let result: serde_json::Value = service.call_tool(name, &args)?;
```

### Runtime error: parameter type mismatch

Make sure the JSON parameter types match the Rust types:

```rust
// Rust: fn add(&self, a: i32, b: i32)
// Wrong: floats
json!({"a": 10.5, "b": 20.5})

// Right: integers
json!({"a": 10, "b": 20})
```

---

## Getting more help

- [Basic usage guide](USAGE.md)
- [AI integration guide](AI_INTEGRATION.md)
- [GitHub issues](https://github.com/silverenternal/tokitai/issues)
