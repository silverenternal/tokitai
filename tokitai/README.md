# Tokitai

[![Crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai)
[![Documentation](https://docs.rs/tokitai/badge.svg)](https://docs.rs/tokitai)
[![License](https://img.shields.io/crates/l/tokitai)](../LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/silverenternal/tokitai/ci.yml)](https://github.com/silverenternal/tokitai/actions)

## One attribute, and your Rust code is AI-callable

```rust
use tokitai::tool;

#[tool]  // <- this is the only line you need!
impl MyTools {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

**Compile-time generation** · **Zero runtime intrusion** · **Type-safe by construction**

---

**Compile-time AI tool definitions · Zero runtime footprint · Magic-sticker integration**

Tokitai is a procedural-macro library with zero runtime dependencies. A single `#[tool]` attribute turns your Rust methods into AI-callable tools, and every tool definition is generated at compile time so type errors surface during compilation.

## 5-minute quick start

### 1. Add the dependency

```toml
[dependencies]
tokitai = "0.5.0"
```

That's it. All required dependencies (serde, serde_json, thiserror) are pulled in automatically.

### 2. Define your tools

```rust
use tokitai::tool;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    /// Add two numbers
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

### 3. Get the tool definitions

```rust
let tools = Calculator::tool_definitions();
```

### 4. Handle an AI call

```rust
use tokitai::json;

let calc = Calculator::default();
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}))?;
println!("{}", result);  // 30
```

## Core features

| Feature | Description |
|---------|-------------|
| **Zero dependency footprint** | Add only `tokitai = "0.5.0"` |
| **Compile-time generation** | Tool definitions are generated during compilation, so type errors surface early |
| **One attribute** | Just `#[tool]` — no chain of annotations to remember |
| **Type-safe by construction** | Rust types are mapped to JSON Schema automatically |
| **Provider-agnostic** | Works with any AI / LLM provider |

## Type mapping

| Rust type | JSON Schema |
|-----------|-------------|
| `String`, `&str` | `string` |
| `i32`, `i64`, `u32`, ... | `integer` |
| `f32`, `f64` | `number` |
| `bool` | `boolean` |
| `Vec<T>` | `array` |
| `Option<T>` | optional `T` |
| user-defined `struct` | `object` |

## Common attributes

```rust
#[tool]
impl MyTools {
    /// Custom name
    #[tool(name = "custom_name")]
    pub fn my_func(&self) {}

    /// Custom description
    #[tool(desc = "Custom description")]
    pub fn another_func(&self) {}

    /// Per-parameter attributes
    pub fn process(
        &self,
        #[tool(desc = "Parameter description", default = "null")]
        options: Option<String>
    ) {}
}
```

## Wrap features (v0.5+)

In addition to the core `#[tool]` macro, the workspace ships a family of **auto-wrapping** macros that turn existing clients, OpenAPI specs, and resilience policies into tools with a single attribute: `#[wrap]`, `#[openapi]` / `#[openapi_op]`, `#[delegate]`, `#[retry]`, `#[rate_limit]`, and `#[circuit_breaker]`. See the [Wrap architecture](../docs/wrap-architecture.md) and the [Wrap cheatsheet](../docs/wrap-cheatsheet.md) for the full picture, and the workspace [README](../README.md#wrap-features-v05) for the at-a-glance summary.

## Documentation

- **[5-minute quick start](../docs/quickstart.md)** — a more detailed getting-started walkthrough
- **[Advanced usage](../docs/ADVANCED_USAGE.md)** — advanced features and best practices
- **[Type system](../docs/USAGE.md)** — how Rust types map to JSON Schema
- **[AI integration](../docs/AI_INTEGRATION.md)** — integrating with AI providers
- **[Architecture](../docs/ARCHITECTURE.md)** — project structure and design
- **[Wrap architecture](../docs/wrap-architecture.md)** — the auto-wrapping macros
- **[API reference](https://docs.rs/tokitai)** — full API documentation

## Requirements

- **Rust version**: 1.80+
- **Edition**: 2021

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE](../LICENSE))
- MIT License ([LICENSE](../LICENSE))

at your option.

## Contributing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual-licensed as above, without any additional terms or conditions.

## Sub-crates

Tokitai is shipped as three crates:

| Crate | Crates.io | Role |
|-------|-----------|------|
| `tokitai` | [![crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai) | Main crate, includes the runtime support |
| `tokitai-core` | [![crates.io](https://img.shields.io/crates/v/tokitai-core.svg)](https://crates.io/crates/tokitai-core) | Core types and traits (zero dependencies) |
| `tokitai-macros` | [![crates.io](https://img.shields.io/crates/v/tokitai-macros.svg)](https://crates.io/crates/tokitai-macros) | Procedural-macro implementation |

**For 99% of users, this is all you need:**

```toml
[dependencies]
tokitai = "0.5.0"
```

## Examples

More examples live under the [examples directory](../examples/):

- `basic_usage.rs` — basic usage
- `quick_chat.rs` — interactive math calculator
- `version_management.rs` — version-management attributes
- `ollama_integration.rs` — Ollama AI integration

---

**Happy coding!**
