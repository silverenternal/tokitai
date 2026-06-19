//! T-016: compile-time and runtime tests for baked few-shot examples.
//!
//! Clippy's `needless_borrows_for_generic_args` sees `call!(...)` as
//! a generic function call and flags the inner literals as
//! needless borrows. They are borrows because the macro re-emits
//! them inside `serde_json::to_value(&(...))` and the type-check
//! site, both of which require references; the lint is therefore
//! a false positive on the macro invocation syntax. We silence
//! it for the whole file.
//!
//! These tests cover:
//!
//! 1. **Positive (3 cases):**
//!    - Single baked example on a sync method.
//!    - Plural baked examples (`examples = [call!(...), call!(...)]`)
//!      on a sync method.
//!    - Baked example on an async method.
//!
//! 2. **Negative (2 cases):** compile-fail when the example's
//!    types do not match the real method's signature. Both
//!    errors must point at the `call!` literal, not at the
//!    generated wrapper.
//!
//! 3. **Schema verification (1 case):** the rendered example
//!    must appear in the output of `to_openai_function()`,
//!    `to_anthropic_tool()`, and `to_mcp_tool()`. The shape
//!    is `{ "input": ..., "output": ... }`.
//!
//! The negative cases are trybuild-driven and live alongside
//!
//!   - `tests/ui/example_baking_wrong_arg.rs`
//!   - `tests/ui/example_baking_wrong_result.rs`
//!
//! with their `.stderr` snapshots under `tests/ui/`.
#![allow(clippy::needless_borrows_for_generic_args)]

use serde_json::json;
use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default, Debug)]
pub struct Calc;

#[tool]
impl Calc {
    /// Add two numbers
    #[tool(example = call!(self.add(1, 2) => 3))]
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Subtract two numbers
    #[tool(examples = [
        call!(self.sub(5, 3) => 2),
        call!(self.sub(10, 7) => 3),
    ])]
    pub fn sub(&self, a: i32, b: i32) -> i32 {
        a - b
    }

    /// Async multiply (still in-process; the type-check works the
    /// same way because the wrapper invokes `self.method_name`).
    #[tool(example = call!(self.mul_async(2, 3) => 6))]
    pub async fn mul_async(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}

// ---------------------------------------------------------------------
// Positive case 1: singular `example = call!(...)` compiles and the
// schema's `examples` field carries the `{ input, output }` envelope.
// ---------------------------------------------------------------------
#[test]
fn test_singular_baked_example_appears_in_schema() {
    let tools = Calc::tool_definitions();
    let add = tools
        .iter()
        .find(|t| t.name == "add")
        .expect("add tool not registered");

    let schema: serde_json::Value =
        serde_json::from_str(&add.input_schema).expect("schema is valid JSON");
    let examples = schema
        .get("examples")
        .and_then(|v| v.as_array())
        .expect("schema must carry an `examples` array");
    assert_eq!(examples.len(), 1, "singular example must produce 1 entry");
    let entry = &examples[0];
    assert!(
        entry.get("input").is_some(),
        "entry must have an `input` key, got: {}",
        entry
    );
    assert!(
        entry.get("output").is_some(),
        "entry must have an `output` key, got: {}",
        entry
    );
    // The literal args `1, 2` serialize to a JSON array `[1, 2]`.
    assert_eq!(entry["input"], json!([1, 2]));
    assert_eq!(entry["output"], json!(3));
}

// ---------------------------------------------------------------------
// Positive case 2: plural `examples = [call!(...), call!(...)]`
// produces one entry per `call!` in declaration order.
// ---------------------------------------------------------------------
#[test]
fn test_plural_baked_examples_appear_in_schema() {
    let tools = Calc::tool_definitions();
    let sub = tools
        .iter()
        .find(|t| t.name == "sub")
        .expect("sub tool not registered");

    let schema: serde_json::Value =
        serde_json::from_str(&sub.input_schema).expect("schema is valid JSON");
    let examples = schema
        .get("examples")
        .and_then(|v| v.as_array())
        .expect("schema must carry an `examples` array");
    assert_eq!(examples.len(), 2, "plural example must produce 2 entries");
    assert_eq!(examples[0]["output"], json!(2));
    assert_eq!(examples[1]["output"], json!(3));
    assert_eq!(examples[0]["input"], json!([5, 3]));
    assert_eq!(examples[1]["input"], json!([10, 7]));
}

// ---------------------------------------------------------------------
// Positive case 3: baked example on an async method. The wrapper
// invokes `self.mul_async(...)` directly inside the type-check
// expression, so this must compile without any async runtime
// installed (the type-check is never executed at runtime).
// ---------------------------------------------------------------------
#[test]
fn test_async_method_accepts_baked_example() {
    let tools = Calc::tool_definitions();
    let m = tools
        .iter()
        .find(|t| t.name == "mul_async")
        .expect("mul_async tool not registered");
    let schema: serde_json::Value =
        serde_json::from_str(&m.input_schema).expect("schema is valid JSON");
    let examples = schema
        .get("examples")
        .and_then(|v| v.as_array())
        .expect("async tool schema must carry an `examples` array");
    assert_eq!(examples.len(), 1);
    assert_eq!(examples[0]["input"], json!([2, 3]));
    assert_eq!(examples[0]["output"], json!(6));
}

// ---------------------------------------------------------------------
// Schema verification: the example must flow through all three
// provider-envelope emitters unchanged. This is the case the
// acceptance criterion explicitly calls out.
// ---------------------------------------------------------------------
#[test]
fn test_example_visible_in_openai_anthropic_mcp() {
    let tools = Calc::tool_definitions();
    let add = tools
        .iter()
        .find(|t| t.name == "add")
        .expect("add tool not registered");

    let openai = add.to_openai_function();
    let openai_examples = openai["function"]["parameters"]["examples"]
        .as_array()
        .expect("OpenAI parameters must carry `examples`");
    assert_eq!(openai_examples.len(), 1);
    assert_eq!(openai_examples[0]["output"], json!(3));

    let anthropic = add.to_anthropic_tool();
    let anthropic_examples = anthropic["input_schema"]["examples"]
        .as_array()
        .expect("Anthropic input_schema must carry `examples`");
    assert_eq!(anthropic_examples.len(), 1);
    assert_eq!(anthropic_examples[0]["output"], json!(3));

    let mcp = add.to_mcp_tool();
    let mcp_examples = mcp["inputSchema"]["examples"]
        .as_array()
        .expect("MCP inputSchema must carry `examples`");
    assert_eq!(mcp_examples.len(), 1);
    assert_eq!(mcp_examples[0]["output"], json!(3));
}
