# Tokitai Usage Guide

**Version**: 0.5.0 | **Last updated**: 2026-06-02

## Table of Contents

1. [Quickstart](#quickstart)
2. [Installation and configuration](#installation-and-configuration)
3. [Basic usage](#basic-usage)
4. [Advanced features](#advanced-features)
5. [Three ways to describe a tool](#three-ways-to-describe-a-tool)
6. [API reference](#api-reference)
7. [Troubleshooting](#troubleshooting)
8. [Best practices](#best-practices)

---

## Quickstart

### 1. Add the dependency

```toml
[dependencies]
tokitai = "0.5.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 2. Define a tool

```rust
use tokitai::tool;

pub struct Calculator;

/// Add two numbers
#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

### 3. Use the tool

```rust
let calc = Calculator::default();

// Get the tool definitions (to send to the AI)
let tools = Calculator::tool_definitions();

// Invoke the tool (in response to an AI request)
let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20}))?;
```

---

## Installation and configuration

### Standard installation

```toml
[dependencies]
tokitai = "0.5.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Minimal installation

```toml
[dependencies]
tokitai = { version = "0.5.0", default-features = false }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Feature flags

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `default` | Enables the full runtime | `serde`, `serde_json`, `thiserror` |
| `serde` | serde serialization support | `serde`, `serde_json` |

### Dependency version requirements

| Dependency | Minimum | Recommended |
|------------|---------|-------------|
| Rust | 1.56.0 | 1.75.0+ |
| serde | 1.0 | 1.0.130+ |
| serde_json | 1.0 | 1.0.70+ |

---

## Basic usage

### Automatic method registration

Annotate the `impl` block with `#[tool]` and every `pub` method is registered as a tool:

```rust
use tokitai::tool;

pub struct WeatherService;

#[tool]
impl WeatherService {
    /// Get the weather for the specified city
    pub fn get_weather(&self, city: String) -> String {
        format!("Weather for {}: clear skies", city)
    }
}
```

### Custom tool attributes

Use `#[tool(name = "...", desc = "...")]` to override the tool's name and description:

```rust
#[tool]
impl WeatherService {
    #[tool(name = "fetch_weather", desc = "Fetch weather data from an external API")]
    pub fn get_weather(&self, city: String) -> String {
        // Call external API...
    }
}
```

### Excluding methods

Use `#[tool(skip)]` to keep internal helpers out of the tool surface:

```rust
#[tool]
impl WeatherService {
    pub fn get_weather(&self, city: String) -> String {
        self.fetch_from_api(city)
    }

    #[tool(skip)]
    fn fetch_from_api(&self, city: &str) -> String {
        // Internal helper, not exposed to the AI
        "API response".to_string()
    }
}
```

### Supported method signatures

#### Synchronous methods

```rust
#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

#### Asynchronous methods

```rust
#[tool]
impl Database {
    pub async fn query(&self, sql: String) -> Result<Vec<Row>, DbError> {
        // Async database query...
    }
}
```

**Note**: asynchronous methods must be awaited with `call_tool().await`.

#### Returning a `Result`

```rust
#[tool]
impl Parser {
    pub fn parse(&self, input: String) -> Result<Data, ParseError> {
        // An operation that may fail...
    }
}
```

---

## Advanced features

### Parameter types in detail

#### Primitive types

| Rust type | JSON Schema | Example value |
|-----------|-------------|---------------|
| `String`, `&str` | `string` | `"hello"` |
| `i8`..=`i128` | `integer` | `42` |
| `u8`..=`u128` | `integer` | `42` |
| `f32`, `f64` | `number` | `3.14` |
| `bool` | `boolean` | `true` |

#### Composite types

| Rust type | JSON Schema | Example value |
|-----------|-------------|---------------|
| `Vec<T>` | `array` | `[1, 2, 3]` |
| `Option<T>` | optional parameter | `null` or a value |
| `HashMap<K, V>` | `object` | `{"key": "value"}` |
| Custom type | `object` | `{"field": "value"}` |

#### Custom types

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

#[tool]
impl MapService {
    pub fn get_weather_at(&self, location: Location) -> String {
        format!("Weather at ({}, {})", location.latitude, location.longitude)
    }
}
```

### Optional parameters

```rust
#[tool]
impl SearchEngine {
    /// Search documents
    pub fn search(
        &self,
        query: String,
        limit: Option<i32>,  // optional parameter
        offset: Option<i32>, // optional parameter
    ) -> Vec<Document> {
        // ...
    }
}
```

### Retrieving tool definitions

```rust
use tokitai::ToolProvider;

// Get all tool definitions (in v0.5.0+ this is a method, not a constant)
let tools = Calculator::tool_definitions();

// Count the tools
let count = Calculator::tool_count();

// Look up a specific tool
if let Some(tool) = Calculator::find_tool("add") {
    println!("Found tool: {}", tool.name);
}
```

### Invoking a tool

#### Synchronous call

```rust
use serde_json::json;

let calc = Calculator::default();
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}));
```

#### Asynchronous call

```rust
let calc = Calculator::default();
let result = calc.call_tool("query", &json!({"sql": "SELECT *"})).await;
```

#### Error handling

```rust
use tokitai::ToolError;

match calc.call_tool("divide", &json!({"a": 10, "b": 0})) {
    Ok(result) => println!("Result: {}", result),
    Err(ToolError { kind: tokitai::ToolErrorKind::ValidationError, message }) => {
        eprintln!("Validation error: {}", message);
    }
    Err(ToolError { kind: tokitai::ToolErrorKind::NotFound, message }) => {
        eprintln!("Tool not found: {}", message);
    }
    Err(e) => eprintln!("Other error: {:?}", e),
}
```

---

## API reference

### Core types

#### `ToolDefinition`

The tool-definition struct; carries a tool's metadata.

```rust
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: &'static str,
}
```

**Methods**:

| Method | Description |
|--------|-------------|
| `new(name, description, input_schema)` | Create a new tool definition |
| `to_json()` | Serialize to a JSON string (requires the `serde` feature) |
| `to_value()` | Convert to a `serde_json::Value` (requires the `serde` feature) |

#### `ToolProvider`

The provider trait. Automatically implemented by the `#[tool]` macro.

```rust
pub trait ToolProvider {
    fn tool_definitions() -> &'static [ToolDefinition];

    fn tool_count() -> usize {
        Self::tool_definitions().len()
    }

    fn find_tool(name: &str) -> Option<&'static ToolDefinition> {
        Self::tool_definitions()
            .iter()
            .find(|t| t.name == name)
    }
}
```

#### `ToolError`

The tool-call error type.

```rust
pub struct ToolError {
    pub kind: ToolErrorKind,
    pub message: String,
}
```

**Error variants**:

| Variant | Value | Description |
|---------|-------|-------------|
| `ToolErrorKind::ValidationError` | 0 | Validation failed |
| `ToolErrorKind::NotFound` | 1 | Tool not found |
| `ToolErrorKind::InternalError` | 2 | Internal error |
| `ToolErrorKind::TypeError` | 3 | Type error |

### Macro attributes

#### `#[tool]` (impl-block level)

Apply to an `impl` block to enable tool registration.

```rust
#[tool]
impl MyStruct {
    // All pub methods are automatically registered as tools
}
```

#### `#[tool(name = "...", desc = "...")]` (method level)

Override the tool's name and description.

```rust
#[tool]
impl MyStruct {
    #[tool(name = "custom_name", desc = "Custom description")]
    pub fn my_method(&self) {}
}
```

#### `#[tool(skip)]` (method level)

Exclude a method so it is not registered as a tool.

```rust
#[tool]
impl MyStruct {
    #[tool(skip)]
    fn internal_helper(&self) {}
}
```

---

## Three ways to describe a tool

Tokitai v0.5.0 supports three flexible ways to describe a tool:

### Method 1: doc comments (recommended)

The simplest approach — use standard Rust doc comments:

```rust
#[tool]
impl MyService {
    /// Get user information
    ///
    /// # Parameters
    /// - `id`: user ID
    /// - `include_profile`: whether to include the full profile
    pub fn get_user(
        &self,
        id: i32,
        include_profile: Option<bool>,
    ) -> User {
        // ...
    }
}
```

**Pros**:
- Standard Rust style
- IDE-friendly
- Zero learning curve

**Cons**:
- The description cannot contain special characters (such as quotes)
- Cannot attach tags or other metadata

### Method 2: `#[tool]` attribute overrides

Finer-grained control:

```rust
#[tool]
impl MyService {
    #[tool(
        desc = "Fetch detailed user information from the database",
        tags = ["user", "read", "database"],
        group = "user_service",
        cache = "ttl=300"
    )]
    pub fn get_user_detail(&self, user_id: i32) -> User {
        // ...
    }

    /// Update a user's profile
    ///
    /// @param id user ID
    /// @param nickname display name
    #[tool(
        example_id = "12345",
        min_length_nickname = 2,
        max_length_nickname = 20,
        pattern_nickname = r"^[a-zA-Z\u4e00-\u9fa5]+$"
    )]
    pub fn update_profile(
        &self,
        id: i32,
        nickname: String,
    ) -> Result<(), Error> {
        // ...
    }
}
```

**Pros**:
- Supports tags, groups, and other metadata
- Per-parameter control
- Supports validation rules

**Cons**:
- A bit more verbose

### Method 3: the `tokitai!` configuration macro

Centralized, batch configuration:

```rust
// Original code stays untouched
impl MyService {
    /// Default description
    pub fn get_user(&self, id: i32) -> User {
        // ...original business logic
    }
}

// Configure everything in one place at the entry point
tokitai::config! {
    MyService {
        get_user: {
            desc: "Configuration-overridden description",
            tags = ["user", "read"],
            params: {
                id: {
                    desc: "Unique user identifier",
                    example: "1001"
                }
            }
        }
    }
}
```

**Pros**:
- Zero changes to existing code
- Centralized management of all tools
- Supports conditional compilation

**Cons**:
- Requires an additional configuration block

## Precedence

The three approaches can be mixed. Precedence is **frozen** (T-002) and
matches the `const fn` table in
[`tokitai_core::config::CONFIG_PRIORITY_ORDER`](https://docs.rs/tokitai-core/latest/tokitai_core/config/constant.CONFIG_PRIORITY_ORDER.html).

| Priority | Source | Layer label | Notes |
|---------:|--------|-------------|-------|
| 1 (highest) | compile-time attribute | `#[tool(desc = "...")]` | **Wins** on conflict; runtime `tokitai!` cannot override. |
| 2 | compile-time doc | `///` doc comment above the method | Used if no `#[tool(desc)]` is present. |
| 3 | runtime registry | `tokitai!` config block | Does **not** override an explicit `#[tool(desc)]`; still wins over the synthesized default. |
| 4 (lowest) | synthesized default | `"调用 <method> 方法"` (or similar) | Last-resort fallback. |

Per-parameter rules:

- `#[param_tool(desc = "...")]` (compile-time) > runtime `tokitai!`
  per-parameter `desc:`. Per-parameter descriptions are independent of
  the tool-level priority table.

If you want runtime overrides to *not* silently trample a compile-time
description, always prefer `#[tool(desc = "...")]` for the parts of your
API contract you want to lock.

> **Why this table is frozen.** Before T-002, the priority was a comment
> inside `tokitai-macros/src/lib.rs`; the user-facing docs and the
> implementation drifted. The table above is rendered from the `const
> fn` `tokitai_core::config::config_priority_table_md()`. Tests pin
> the behaviour; see `tokitai/tests/config_override_test.rs`.

## Best practices

- **Simple cases**: use doc comments
- **Complex parameters**: use `#[tool(...)]` per-parameter attributes
- **Batch management**: use the `tokitai!` configuration macro

---

## Troubleshooting

### Compilation errors

#### Error: generic methods are not supported

```
error: Generic methods are not supported
  = help: Remove generic parameters or use concrete types
```

**Cause**: the `#[tool]` macro does not support generic methods.

**Fix**: replace the generic parameters with concrete types.

```rust
// Does not compile
#[tool]
impl MyTools {
    pub fn process<T: Serialize>(&self, data: T) -> String {
        // ...
    }
}

// Compiles
#[tool]
impl MyTools {
    pub fn process_string(&self, data: String) -> String {
        // ...
    }

    pub fn process_json(&self, data: serde_json::Value) -> String {
        // ...
    }
}
```

#### Error: missing `serde` feature

```
error[E0433]: failed to resolve: use of undeclared crate or module `serde_json`
```

**Cause**: the `serde` feature is not enabled.

**Fix**: enable the feature in `Cargo.toml`.

```toml
[dependencies]
tokitai = { version = "0.5.0", features = ["serde"] }
serde_json = "1.0"
```

### Runtime errors

#### Error: async method called without a runtime

```
Error: async tool calls require a tokio runtime
```

**Cause**: async tool methods need a tokio runtime.

**Fix**: use `#[tokio::main]` or `tokio::runtime::Runtime`.

```rust
#[tokio::main]
async fn main() {
    let calc = Calculator::default();
    let result = calc.call_tool("async_method", &args).await;
}
```

#### Error: tool not found

```
Error: Tool not found: unknown_tool
```

**Cause**: you called a tool that does not exist.

**Fix**: double-check the tool name.

```rust
// Print all available tools
for tool in MyTools::tool_definitions() {
    println!("Available tool: {}", tool.name);
}
```

### Common questions

#### Q: How do I debug a tool call?

**A**: enable logging:

```rust
// Cargo.toml
[dependencies]
env_logger = "0.10"

// main.rs
fn main() {
    env_logger::init();
    // ...
}
```

#### Q: How do I see the macro-generated code?

**A**: use `cargo expand`:

```bash
cargo install cargo-expand
cargo expand --example basic_usage
```

#### Q: Which AI platforms are supported?

**A**: the tool definitions Tokitai generates are vendor-neutral. They are compatible with any AI platform that supports function calling, including:

- Ollama (local or cloud)
- Claude
- GPT-4
- Any other OpenAI-compatible platform

---

## Best practices

### 1. Tool naming

- Use verb + noun: `get_weather`, `create_user`, `delete_file`
- Avoid abbreviations unless they are well established
- Keep naming consistent

### 2. Doc comments

Write clear doc comments for every tool method:

```rust
#[tool]
impl Calculator {
    /// Add two integers and return the result.
    ///
    /// # Parameters
    /// - `a`: the first integer
    /// - `b`: the second integer
    ///
    /// # Returns
    /// The sum of the two integers
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

### 3. Error handling

Return a `Result` for fallible operations:

```rust
#[tool]
impl FileService {
    pub fn read_file(&self, path: String) -> Result<String, String> {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read file: {}", e))
    }
}
```

### 4. Grouping tools

Organize related tools into the same struct:

```rust
// User-management tools
pub struct UserService { /* ... */ }

#[tool]
impl UserService {
    pub fn create_user(&self, name: String) -> User { /* ... */ }
    pub fn get_user(&self, id: i32) -> Option<User> { /* ... */ }
    pub fn delete_user(&self, id: i32) -> bool { /* ... */ }
}
```

### 5. Performance considerations

- Keep tool methods lightweight; avoid long blocking operations
- Use async methods for slow I/O
- Cache results when the same computation is repeated

---

## Example code

More examples:

- [`examples/basic_usage.rs`](../examples/basic_usage.rs) - Basic usage
- [`examples/ollama_integration.rs`](../examples/ollama_integration.rs) - Ollama integration
- [`examples/starter_project/`](../examples/starter_project/) - Full project template

---

## Related links

- [README](../README.md) - Project home
- [AI integration guide](AI_INTEGRATION.md) - Integrating with AI platforms
- [Architecture](ARCHITECTURE.md) - Internal design notes
- [API documentation](https://docs.rs/tokitai) - Rust API reference
