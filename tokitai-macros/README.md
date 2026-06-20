# Tokitai Macros

[![Crates.io](https://img.shields.io/crates/v/tokitai-macros.svg)](https://crates.io/crates/tokitai-macros)
[![Documentation](https://docs.rs/tokitai-macros/badge.svg)](https://docs.rs/tokitai-macros)
[![License](https://img.shields.io/crates/l/tokitai-macros)](../LICENSE)

## Procedural Macro Implementation

Tokitai Macros ships the `#[tool]` procedural macro, which generates AI tool definitions at compile time. The macro itself has no runtime dependencies — every byte of code it produces is emitted during compilation.

## Core Features

- **Zero runtime dependencies** — the macro itself has no runtime overhead.
- **Compile-time generation** — tool definitions are produced during compilation.
- **Type safety** — parameter validation happens at compile time.
- **Automatic discovery** — after tagging an `impl` block, every `pub` method becomes a tool.
- **Customizable** — tool names and descriptions can be overridden via attributes.
- **Provider-agnostic** — works with any AI / LLM provider.

## Quick Start

### Add the Dependency

```toml
[dependencies]
tokitai = "0.6"
```

**Note**: you usually do not need to add `tokitai-macros` directly. It is re-exported by the `tokitai` crate.

### Basic Usage

```rust
use tokitai::tool;

pub struct Calculator;

#[tool]
impl Calculator {
    /// Add two numbers
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Multiply two numbers
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}

// Use it
let calc = Calculator::default();

// Retrieve the tool definitions (generated at compile time)
let tools = Calculator::tool_definitions();
println!("Number of tools: {}", tools.len());

// Invoke a tool
let result = calc.call_tool("add", &tokitai::json!({"a": 10, "b": 20})).unwrap();
println!("Result: {}", result);  // 30
```

## What the Macro Does

`#[tool]` automatically handles the following:

1. **Extracts doc comments** to use as tool descriptions.
2. **Generates a JSON Schema** by mapping each Rust parameter type to its JSON counterpart.
3. **Creates a `tool_definitions()` method** that exposes the full tool metadata.
4. **Implements a `call_tool` dispatcher** for runtime invocation.
5. **Generates argument-parsing code** to validate and convert parameter types automatically.

## Type Mapping

Rust types are mapped to JSON Schema types automatically.

### Primitive Types

| Rust type | JSON Schema type |
|-----------|------------------|
| `String`, `&str` | `string` |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | `integer` |
| `f32`, `f64` | `number` |
| `bool` | `boolean` |

### Compound Types

| Rust type | JSON Schema type |
|-----------|------------------|
| `Vec<T>` | `array` |
| `Option<T>` | Optional parameter |
| `HashMap<K, V>` | `object` |
| Custom struct | `object` |

## Attribute Syntax

### 1. Tagging an `impl` block (recommended)

```rust
#[tool]
impl MyTools {
    // Every `pub` method is automatically registered as a tool.
}
```

### 2. Customizing a tool's name and description

```rust
#[tool]
impl MyTools {
    #[tool(name = "fetch_url", desc = "Fetch content from a URL")]
    pub fn fetch(&self, url: String) -> String {
        // implementation
    }
}
```

### 3. Excluding a method

```rust
#[tool]
impl MyTools {
    pub fn public_tool(&self) {}

    #[tool(skip)]
    fn internal_helper(&self) {}  // not registered as a tool
}
```

### 4. Parameter-level attributes

```rust
#[tool]
impl MyTools {
    pub fn process(
        &self,
        #[tool(desc = "Parameter description", default = "null")]
        options: Option<String>
    ) {}
}
```

## Generated Code

For every `#[tool]` `impl` block, the macro expands into something similar to the following (simplified for clarity):

```rust
// 1. The tool-definition method
impl Calculator {
    pub fn tool_definitions() -> &'static [ToolDefinition] {
        &[
            ToolDefinition {
                name: "add",
                description: "Add two numbers",
                input_schema: "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}",
            },
            // ... other tools
        ]
    }
}

// 2. The `ToolProvider` trait implementation
impl ToolProvider for Calculator {
    fn tool_definitions() -> &'static [ToolDefinition] {
        Self::tool_definitions()
    }
}

// 3. A `__call_<name>` wrapper per tool, holding all argument parsing,
//    validation, and conversion logic.
impl Calculator {
    fn __call_add(&self, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> {
        // generated argument parsing for `a` and `b`
        let a: i32 = /* ... */;
        let b: i32 = /* ... */;
        let result = self.add(a, b);
        Ok(serde_json::to_value(result).unwrap())
    }

    fn __call_multiply(&self, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> {
        // generated argument parsing for `a` and `b`
        let result = self.multiply(a, b);
        Ok(serde_json::to_value(result).unwrap())
    }
}

// 4. The `call_tool` dispatcher
impl Calculator {
    pub fn call_tool(&self, name: &str, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> {
        match name {
            "add" => self.__call_add(args),
            "multiply" => self.__call_multiply(args),
            _ => Err(::tokitai::ToolError::not_found("unknown tool")),
        }
    }
}
```

In addition, when the `impl` block contains any `async` method, the macro also emits a `call_tool_sync` dispatcher and `__call_<name>_sync` wrappers so that synchronous callers can still reach every tool.

## Performance

| Operation | Time |
|-----------|------|
| Macro compilation | < 50 ms |
| Tool-definition generation | Zero runtime cost (compile-time) |
| `call_tool` dispatch | < 1 μs |

> Benchmarked on Rust 1.75, M1 Pro, 16 GB RAM.

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
| `tokitai-core` | [![crates.io](https://img.shields.io/crates/v/tokitai-core.svg)](https://crates.io/crates/tokitai-core) | Core types and traits. |

## Documentation

- **[API Reference](https://docs.rs/tokitai-macros)** — complete API documentation.
- **[Usage Guide](../docs/USAGE.md)** — detailed walk-throughs.
- **[Advanced Usage](../docs/ADVANCED_USAGE.md)** — advanced features and best practices.
- **[Architecture](../docs/ARCHITECTURE.md)** — notes on the macro's internal design.

---

**Happy Coding!**
