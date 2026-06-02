# 5-Minute Quickstart

## 1. Add the Dependency

```toml
[dependencies]
tokitai = "0.5.0"
serde_json = "1.0"
```

That's all you need. Every transitive dependency (`serde`, `thiserror`) is pulled in automatically; you only need to add `serde_json` to construct and parse the JSON payloads.

## 2. Define a Tool

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

    /// Multiply two numbers
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
```

## 3. Get the Tool Definitions (to Send to the AI)

```rust
// Compile-time generated tool definitions
let tools = Calculator::tool_definitions();

// Convert to JSON and send to the AI
let json = serde_json::json!({
    "tools": tools.iter().map(|t| {
        serde_json::json!({
            "name": t.name,
            "description": t.description,
            "input_schema": t.input_schema
        })
    }).collect::<Vec<_>>()
});

println!("{}", serde_json::to_string_pretty(&json)?);
```

Output:

```json
[
  {
    "name": "add",
    "description": "Add two numbers",
    "input_schema": "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}"
  },
  {
    "name": "multiply",
    "description": "Multiply two numbers",
    "input_schema": "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}"
  }
]
```

## 4. Handle a Tool Call From the AI

```rust
use serde_json::json;

let calc = Calculator::default();

// The AI returns a tool-call request
let call_request = json!({
    "name": "add",
    "arguments": {"a": 10, "b": 20}
});

// Execute the tool call
let result = calc.call_tool(
    call_request["name"].as_str().unwrap(),
    &call_request["arguments"]
)?;

println!("Result: {}", result);  // 30
```

## Full Example

Run the included example to see it work end to end:

```bash
# Basic usage
cargo run --example basic_usage

# MCP server example
cargo run --example mcp_builder_demo -p tokitai-mcp-server

# Multi-tool chat
cargo run --example multi_tool_chat
```

## Next Steps

- [Full API documentation](https://docs.rs/tokitai)
- [More examples](https://github.com/silverenternal/tokitai/tree/main/examples)
- [Advanced usage](ADVANCED_USAGE.md)
- [Type mapping](USAGE.md)

## Frequently Asked Questions

### Why do I only need a single `#[tool]` attribute?

Tokitai analyzes your code at compile time and generates every piece of metadata for you. You do not need a separate `#[tool_name]`, `#[tool_description]`, or any other attribute.

### Warnings about `Option` Parameters

If a parameter has type `Option<T>`, it is good practice to add a default value or an example so the AI knows it can be omitted:

```rust
#[tool]
impl MyTools {
    // Add a default value
    pub fn process(&self, data: String, #[tool(default = "null")] options: Option<serde_json::Value>) {
        // ...
    }

    // Or make it required
    pub fn process(&self, data: String, options: serde_json::Value) {
        // ...
    }
}
```

### How Do I Customize a Tool's Name?

```rust
#[tool]
impl MyTools {
    #[tool(name = "custom_name")]
    pub fn my_function(&self, x: i32) -> i32 {
        x * 2
    }
}
```

### Which Types Are Supported?

Rust types are mapped to JSON Schema automatically:

| Rust type | JSON Schema |
|-----------|-------------|
| `String`, `&str` | `string` |
| `i32`, `i64`, `u32`, etc. | `integer` |
| `f32`, `f64` | `number` |
| `bool` | `boolean` |
| `Vec<T>` | `array` |
| Custom struct | `object` |
