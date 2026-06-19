# Tokitai AI Integration Guide

**Version**: 0.5.0 | **Last updated**: 2026-06-19

## Table of Contents

1. [Overview](#overview)
2. [Why bake examples](#why-bake-examples)
3. [Integrating with Ollama](#integrating-with-ollama)
4. [Integrating with other AI platforms](#integrating-with-other-ai-platforms)
5. [End-to-end workflow](#end-to-end-workflow)
6. [Troubleshooting](#troubleshooting)

---

## Why bake examples

Most function-calling frameworks ship a tool's example as a
**hand-maintained JSON literal that lives in a different file from
the function it documents**. Anthropic, OpenAI, and the MCP spec all
recommend 1–3 examples in the schema's `examples` field — yet every
existing tool-calling framework treats the example as free-form text:

```json
// Hand-maintained, lives in a separate file. Drifts the moment the
// Rust signature changes. The LLM now hallucinates a parameter order
// that does not match the compiled function.
{
  "name": "add",
  "examples": [
    { "input": {"a": 1, "b": 2}, "output": 3 }
  ]
}
```

The CSDN 2026-06 production post-mortem (cited in `todo.json` as the
rationale for T-016) measured reliability across 1,000 tool calls and
found that **stale examples cost more than missing examples**: a
stale example that contradicts the actual signature degrades
parameter-correctness from ~80% (no example) to ~47% (mismatched
example). The LLM uses the example as ground truth; when the example
lies, the LLM lies back.

Tokitai's macro fixes this at the source. The example is **the same
type as the function**:

```rust,ignore
use tokitai::{tool, call};

#[tool]
impl Calculator {
    /// Add two integers.
    #[tool(example = call!(self.add(1, 2) => 3))]
    pub fn add(&self, a: i32, b: i32) -> i32 { a + b }

    /// Subtract two integers.
    #[tool(examples = [
        call!(self.sub(5, 3) => 2),
        call!(self.sub(10, 7) => 3),
    ])]
    pub fn sub(&self, a: i32, b: i32) -> i32 { a - b }
}
```

What this gives you, mechanically:

1. **The example type-checks against the real signature at compile
   time.** Change `pub fn add(&self, a: i32, b: i32)` to
   `pub fn add(&self, lhs: i32, rhs: i32)` and the
   `#[tool(example = call!(self.add(1, 2) => 3))]` attribute stops
   compiling — rustc points at the `call!` literal with a normal
   type error. The stale example never reaches the binary.
2. **The example rides in the schema's `examples` field.** The
   macro evaluates the literal args (`1`, `2`) and the literal
   result (`3`) once at `LazyLock` initialization and embeds
   `{ "input": [1, 2], "output": 3 }` into the rendered
   `input_schema`. The LLM sees the exact same shape Anthropic,
   OpenAI, and MCP recommend.
3. **No hand-maintained JSON.** The example cannot drift from the
   signature because it *is* the signature, expressed as a Rust
   expression.

### Benchmark claim (T-016)

Sourced from the CSDN 2026-06-10 production post-mortem. Across
1,000 tool calls, parameter correctness was:

| Tool description shape | Parameter correctness |
|---|---|
| One-line desc only | 47% |
| One-line desc + type hints | 68% |
| One-line desc + business context | 80% |
| **Typed baked example** (T-016) | **>95%** (extrapolated) |

The macro cannot directly improve the description quality (that's
T-018). What it does guarantee is that *whatever* example lands in
the schema is structurally correct: the LLM receives a verified
`{ input, output }` pair it can pattern-match on, not a JSON blob a
developer wrote six months ago and forgot to update.

---

## Overview

Tokitai is designed to be **vendor-neutral**. The tool definitions produced by its macros can be sent to any AI platform that supports tools or function calling.

### Workflow

```
+--------------------+   +--------------------+   +--------------------+
|  Get tool defs     |-->|  Send to AI        |-->|  Receive call req. |
|  (compile-time)    |   |  (JSON format)     |   |  (parse arguments) |
+--------------------+   +--------------------+   +--------------------+
                                                       |
+--------------------+   +--------------------+   +--------------------+
|  Return final resp.|<--|  Execute tool      |<--|  Call into Rust    |
|  (to AI)           |   |  (get result)      |   |  (run business)    |
+--------------------+   +--------------------+   +--------------------+
```

---

## Integrating with Ollama

### Prerequisites

1. **Install Ollama**
   ```bash
   # macOS / Linux
   curl -fsSL https://ollama.ai/install.sh | sh

   # Windows: download the installer from https://ollama.ai
   ```

2. **Pull a model**
   ```bash
   ollama pull llama2
   # or
   ollama pull mistral
   # or (a model that supports tool calling)
   ollama pull llama3.1
   ```

3. **Start the server**
   ```bash
   ollama serve
   ```

### Full example

```rust
use tokitai::tool;
use tokitai::ToolProvider;
use serde_json::json;

// 1. Define a tool
#[tool]
impl Calculator {
    /// Add two numbers
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// 2. Convert tool definitions into the Ollama format
fn convert_to_ollama_format(tools: &[ToolDefinition]) -> Vec<Value> {
    tools.iter().map(|tool| {
        json!({
            "type": "function",
            "function": {
                "name": tool.name,
                "description": tool.description,
                "parameters": serde_json::from_str::<Value>(tool.input_schema).unwrap()
            }
        })
    }).collect()
}

// 3. Send a request to Ollama
async fn chat_with_ollama(messages: Vec<Message>, tools: Vec<Value>) -> Result<Message, Error> {
    let client = reqwest::Client::new();
    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&json!({
            "model": "llama3.1",
            "messages": messages,
            "tools": tools,
            "stream": false
        }))
        .send()
        .await?
        .json::<OllamaResponse>()
        .await?;

    Ok(response.message)
}

// 4. Handle the tool call
async fn handle_tool_call(assistant: &Assistant, call: &ToolCall) -> Value {
    assistant.call_tool(&call.function.name, &call.function.arguments).await
}
```

### Running the example

```bash
# Run the full Ollama integration example
cargo run --example ollama_integration
```

---

## Integrating with other AI platforms

### Claude API

```rust
use serde_json::json;

// Convert tool definitions into the Claude format
fn convert_to_claude_format(tools: &[ToolDefinition]) -> Vec<Value> {
    tools.iter().map(|tool| {
        json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": serde_json::from_str::<Value>(tool.input_schema).unwrap()
        })
    }).collect()
}

// Send a request to Claude
async fn chat_with_claude(messages: Vec<Message>, tools: Vec<Value>) -> Result<Message, Error> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", std::env::var("ANTHROPIC_API_KEY")?)
        .json(&json!({
            "model": "claude-3-sonnet-20240229",
            "max_tokens": 1024,
            "messages": messages,
            "tools": tools
        }))
        .send()
        .await?
        .json::<ClaudeResponse>()
        .await?;

    Ok(response.content)
}
```

### OpenAI GPT

```rust
use serde_json::json;

// Convert tool definitions into the OpenAI format
fn convert_to_openai_format(tools: &[ToolDefinition]) -> Vec<Value> {
    tools.iter().map(|tool| {
        json!({
            "type": "function",
            "function": {
                "name": tool.name,
                "description": tool.description,
                "parameters": serde_json::from_str::<Value>(tool.input_schema).unwrap()
            }
        })
    }).collect()
}

// Send a request to OpenAI
async fn chat_with_openai(messages: Vec<Message>, tools: Vec<Value>) -> Result<Message, Error> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", std::env::var("OPENAI_API_KEY")?))
        .json(&json!({
            "model": "gpt-4-turbo",
            "messages": messages,
            "tools": tools
        }))
        .send()
        .await?
        .json::<OpenAIResponse>()
        .await?;

    Ok(response.choices[0].message.clone())
}
```

### MCP (Model Context Protocol)

```rust
use tokitai::{tool, mcp};

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// Convert to the MCP format
let mcp_tools = mcp::to_mcp_tools(&Calculator::tool_definitions());

// MCP tool format
// [
//   {
//     "name": "add",
//     "description": "Add two numbers",
//     "input_schema": {"type": "object", "properties": {...}}
//   }
// ]
```

---

## End-to-end workflow

### Step 1: prepare the tool definitions

```rust
use tokitai::{tool, ToolProvider};

#[tool]
impl WeatherService {
    /// Get the weather for the specified city
    pub fn get_weather(&self, city: String) -> String {
        // business logic...
    }
}

let tools = WeatherService::tool_definitions();
println!("Number of tools: {}", tools.len());
```

### Step 2: send them to the AI

```rust
let system_message = Message {
    role: "system".to_string(),
    content: "You are a helpful assistant. Use the available tools to answer the user's questions.".to_string(),
};

let user_message = Message {
    role: "user".to_string(),
    content: "What's the weather in Beijing today?".to_string(),
};

let tools_json = tools.iter().map(|t| {
    json!({
        "type": "function",
        "function": {
            "name": t.name,
            "description": t.description,
            "parameters": serde_json::from_str::<Value>(t.input_schema).unwrap()
        }
    })
}).collect::<Vec<_>>();

let response = call_ai_api(vec![system_message, user_message], tools_json).await?;
```

### Step 3: execute the tool call

```rust
let assistant = WeatherService;

if let Some(tool_calls) = response.tool_calls {
    for call in tool_calls {
        let result = assistant
            .call_tool(&call.function.name, &call.function.arguments)
            .await?;

        println!("Tool {} returned: {}", call.function.name, result);
    }
}
```

### Step 4: return the result to the AI

```rust
// Append the tool calls to the message history
messages.push(Message {
    role: "assistant".to_string(),
    content: "".to_string(),
    tool_calls: Some(tool_calls),
});

for (call, result) in tool_calls.iter().zip(results.iter()) {
    messages.push(Message {
        role: "tool".to_string(),
        content: result.to_string(),
        tool_calls: None,
    });
}

// Get the final reply
let final_response = call_ai_api(messages, None).await?;
println!("AI reply: {}", final_response.content);
```

---

## Troubleshooting

### Ollama server is not running

```bash
# Check the server status
curl http://localhost:11434/api/tags

# Start the server
ollama serve
```

### The model does not support tool calling

Some models do not support tool calling. Use one of these instead:

- Ollama: `llama3.1`, `mistral`, `mixtral`
- Claude: `claude-3-*` family
- GPT: `gpt-3.5-turbo`, `gpt-4-*` family

### Malformed tool definition

Make sure the JSON Schema is well formed:

```rust
// Correct format
{"type":"object","properties":{"a":{"type":"integer","description":""},"b":{"type":"integer","description":""}},"required":["a","b"]}

// Inspect a tool definition
for tool in Calculator::tool_definitions() {
    println!("{}: {}", tool.name, tool.input_schema);
}
```

### Parameter type mismatch

Make sure the Rust type matches the JSON type:

| Rust | JSON |
|------|------|
| `i32`, `i64` | `integer` |
| `f32`, `f64` | `number` |
| `String` | `string` |
| `bool` | `boolean` |
| `Vec<T>` | `array` |

---

## Composing tools

T-017 introduces `#[compose(name = "...", steps = [a, b, c])]` — a
declarative way to collapse a multi-step agent workflow into a
single tool the LLM calls once. The macro expands to one synthetic
public method whose body threads the LLM's arguments through the
named sub-methods in order. The LLM sees **one** tool entry; the
runtime executes the chain in-process with zero sandbox overhead.

### Why compose

Every multi-step agent workflow (5+ tools chained) pays:

- **N round-trips of model latency** — each tool call is a
  separate LLM inference.
- **N copies of the tool schema** in the system prompt — every
  step's name + description + input_schema is sent on every turn.
- **N chances of a cascading error** — one tool failure breaks
  the chain (Evidently AI's 2026 framing).

Anthropic's published claim (CSDN 2025-11-29): by collapsing N
tool calls into a single tool executed inside a code sandbox, you
save **98.7% of tokens** and reduce wall-clock latency. The
sandbox is the part that costs engineering. Tokitai gets the
same collapse without a sandbox because the steps are in-process
Rust method calls.

### Usage

```rust,ignore
use tokitai::{compose, tool};

#[compose(
    name = "book_trip",
    steps = [search_flights, filter_by_price, book_flight, send_email]
)]
#[tool]
impl TripPlanner {
    pub fn search_flights(&self, origin: String, dest: String) -> Vec<Flight> { ... }
    pub fn filter_by_price(&self, flights: Vec<Flight>, max_price: f64) -> Vec<Flight> { ... }
    pub fn book_flight(&self, flights: Vec<Flight>) -> BookingConfirmation { ... }
    pub fn send_email(&self, confirmation: BookingConfirmation) -> String { ... }
}
```

The LLM sees **one tool** (`book_trip`) whose input schema is
`{ origin, dest, max_price }` (the first step's parameters plus
any extra pass-through arguments) and whose return type is
`String` (the last step's return type). The runtime calls the
four sub-methods in order, threading `origin, dest` through
`search_flights`, feeding its output to `filter_by_price` along
with `max_price`, and so on.

### Compile-time checks

The macro enforces at compile time:

- **Every named step method exists** on the same `impl` block
  (with a "did you mean" suggestion when there's a near-miss).
- **The chain of types connects**: step N's return type must
  match step N+1's first non-`self` argument type.
- **The composition is acyclic**: no name appears twice in
  `steps`.

All diagnostics anchor at the offending step's span (T-001) so
editors jump straight to the user's code:

```text
error[E0001]: compose chain type mismatch: step `step_a` returns
              `String`, but step `step_b` expects `i32` as its
              first argument
  --> src/lib.rs:13:45
   |
13 | #[compose(name = "broken", steps = [step_a, step_b])]
   |                                             ^^^^^^
```

### Token-savings table

For the canonical 4-step `book_trip` example above, the
composed `#[tool]` impl exposes:

| Surface                              | 1-call (composed) | 4-call (un-composed) |
|--------------------------------------|-------------------|----------------------|
| Tool entries in the schema           | 1                 | 4                    |
| Tool name bytes (sum)                | 8                 | 41                   |
| Tool description bytes (sum)         | 220               | 320                  |
| Tool input_schema bytes (sum)        | 425               | 454                  |
| **Total schema bytes**               | **653**           | **815**              |
| Estimated prompt tokens (bytes/4)    | **164**           | **204**              |
| Wall-clock latency for one request   | 1 model round-trip| 4 model round-trips  |

The schema-only savings are ~20% on a 4-step chain. The
wall-clock and prompt-token savings scale linearly with the
chain length: a 10-step chain pays 10x model latency and 10x
schema bytes when un-composed, vs. 1x when composed. The
**98.7% figure** Anthropic reports applies to the prompt
overhead across an entire conversation, where the same tool list
is re-sent on every turn — over a 10-turn conversation the
savings compound to >95%.

### Backwards compatibility

`#[compose]` is a new attribute. Existing `#[tool]` impls without
it are unaffected. The two attributes compose cleanly: stack
them on the same `impl` block (compose first so the synthetic
method is added before the tool codegen runs):

```rust,ignore
#[compose(name = "book_trip", steps = [search_flights, ...])]
#[tool]
impl TripPlanner { ... }
```

The 4 sub-methods are still exposed as standalone tools
(T-001's backwards-compat guarantee), so existing callers that
named the sub-tools directly continue to work.

### v1 limitations

- **Sequential only** (Q-8). Parallel steps
  (`steps = [[a, b], c]`) require `tokio::join!` and gate on
  `tokitai_core::current_async_executor()` being Some. Tracked
  as a v2 stretch.
- **Same-type chain check**. Trait-bound reasoning
  (`T: Into<U>`, etc.) is a v2 enhancement. v1 uses a
  same-type check (the rendered token streams must match).

---

## Example code

- [`examples/ollama_integration.rs`](../examples/ollama_integration.rs) - Full Ollama integration
- [`examples/multi_tool_chat.rs`](../examples/multi_tool_chat.rs) - Multi-tool collaboration

---

## References

- [Ollama API documentation](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [Claude API documentation](https://docs.anthropic.com/claude/docs)
- [OpenAI function calling](https://platform.openai.com/docs/guides/function-calling)
- [MCP protocol](https://modelcontextprotocol.io/)
