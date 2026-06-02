# Tokitai Core

[![Crates.io](https://img.shields.io/crates/v/tokitai-core.svg)](https://crates.io/crates/tokitai-core)
[![Documentation](https://docs.rs/tokitai-core/badge.svg)](https://docs.rs/tokitai-core)
[![License](https://img.shields.io/crates/l/tokitai-core)](../LICENSE)

## Core Type Definitions

Tokitai Core provides the foundational types and traits that underpin the Tokitai AI tool integration system. All tool metadata is generated at compile time, which guarantees zero runtime overhead and maximum type safety.

## Core Features

- **Zero runtime dependencies** — the core types are kept as lean as possible.
- **Type-safe** — tool definitions are produced at compile time, eliminating a class of runtime errors.
- **Serde integration** — optional serialization support is available behind the `serde` feature.

## Core Types

| Type | Description |
|------|-------------|
| [`ToolDefinition`] | A tool definition, including name, description, and input schema. |
| [`ToolParameter`] | Definition of a tool parameter. |
| [`ParamType`] | Enumeration of JSON Schema types. |
| [`ToolError`] | Error type returned by tool invocations. |
| [`ToolErrorKind`] | Enumeration used to classify errors. |
| [`ToolProvider`] | Trait implemented by tool providers (auto-implemented by the `#[tool]` macro). |

## Quick Start

### Add the Dependency

```toml
[dependencies]
tokitai-core = "0.5"
```

### Basic Usage

```rust
use tokitai_core::ToolDefinition;

// Create a tool definition
let tool = ToolDefinition::new(
    "add",
    "Add two numbers",
    r#"{"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}},"required":["a","b"]}"#
);

assert_eq!(tool.name, "add");
assert_eq!(tool.description, "Add two numbers");

// When the `serde` feature is enabled, the definition can be serialized to JSON.
#[cfg(feature = "serde")]
{
    let json = tool.to_json().unwrap();
    println!("{}", json);
}
```

## Type Mapping

The [`ParamType`] enum maps Rust types to their JSON Schema counterparts:

| Rust type | JSON Schema type | `ParamType` variant |
|-----------|------------------|---------------------|
| `String`, `&str` | `string` | `ParamType::String` |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | `integer` | `ParamType::Integer` |
| `f32`, `f64` | `number` | `ParamType::Number` |
| `bool` | `boolean` | `ParamType::Boolean` |
| `Vec<T>` | `array` | `ParamType::Array` |
| Custom struct | `object` | `ParamType::Object` |

```rust
use tokitai_core::ParamType;

assert_eq!(ParamType::from_rust_type("String"), Some(ParamType::String));
assert_eq!(ParamType::from_rust_type("i32"), Some(ParamType::Integer));
assert_eq!(ParamType::from_rust_type("f64"), Some(ParamType::Number));
assert_eq!(ParamType::from_rust_type("bool"), Some(ParamType::Boolean));
assert_eq!(ParamType::from_rust_type("Vec<i32>"), Some(ParamType::Array));
```

## Error Handling

The [`ToolError`] type provides structured error reporting for tool invocations:

```rust
use tokitai_core::{ToolError, ToolErrorKind};

// Build different kinds of errors
let validation_err = ToolError::validation_error("missing required parameter 'city'");
assert_eq!(validation_err.kind, ToolErrorKind::ValidationError);

let not_found_err = ToolError::not_found("tool 'unknown_tool' not found");
assert_eq!(not_found_err.kind, ToolErrorKind::NotFound);

let internal_err = ToolError::internal_error("connection timed out");
assert_eq!(internal_err.kind, ToolErrorKind::InternalError);
```

### Error Variants

| Variant | Value | Description |
|---------|-------|-------------|
| `ToolErrorKind::ValidationError` | 0 | Parameter validation failed. |
| `ToolErrorKind::NotFound` | 1 | The requested tool was not found. |
| `ToolErrorKind::InternalError` | 2 | An internal error occurred. |
| `ToolErrorKind::TypeError` | 3 | A type error occurred. |

## The `ToolProvider` Trait

The [`ToolProvider`] trait is automatically implemented by the `#[tool]` macro:

```rust
use tokitai_core::ToolProvider;

// Once you have applied `#[tool]` to your type:
// struct Calculator;
// #[tool] impl Calculator { ... }

// Retrieve every tool definition
let tools = Calculator::tool_definitions();

// Count the registered tools
let count = Calculator::tool_count();

// Look up a specific tool by name
let tool = Calculator::find_tool("add");
```

## The `json_schema!` Macro

The `json_schema!` macro helps you build JSON Schema strings at compile time:

```rust,ignore
use tokitai_core::json_schema;

const SCHEMA: &str = json_schema!({
    "city": {
        type: String,
        description: "Name of the city",
        required: true,
    }
});
```

## Features

| Feature | Description |
|---------|-------------|
| `serde` (enabled by default) | Enables serde-based serialization and JSON support. |

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

## Related Crates

| Crate | Crates.io | Description |
|-------|-----------|-------------|
| `tokitai` | [![crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai) | Main crate, bundling runtime support. |
| `tokitai-macros` | [![crates.io](https://img.shields.io/crates/v/tokitai-macros.svg)](https://crates.io/crates/tokitai-macros) | Procedural macro implementation. |

## Documentation

- **[API Reference](https://docs.rs/tokitai-core)** — complete API documentation.
- **[Usage Guide](../docs/USAGE.md)** — detailed walk-throughs.
- **[Architecture](../docs/ARCHITECTURE.md)** — internal design notes.

---

**Happy Coding!**
