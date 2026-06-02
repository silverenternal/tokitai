//! Tests for the `enum_values_*` method-level attribute prefix.
//!
//! `enum_values_*` is one of the validation prefixes that
//! `tokitai-macros::tool::attrs::method::MethodToolAttrs::parse`
//! recognises. It accepts a bracketed list of arbitrary expressions
//! (unlike `one_of_*` which only accepts string literals) and stores
//! them in `ParamToolAttrs::enum_values`. The schema generator reads
//! that field and emits a JSON Schema `enum` with each value
//! stringified.
//!
//! Note: at runtime the macro only enforces `one_of_*`, not
//! `enum_values_*`. The latter is purely a schema-emission
//! attribute. The tests below therefore only assert the JSON Schema
//! shape, not runtime rejection.

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct EnumValueTools;

#[tool]
impl EnumValueTools {
    /// String-typed enum.
    #[tool(enum_values_color = ["red", "green", "blue"])]
    pub fn pick_color(&self, color: String) -> String {
        format!("picked {}", color)
    }

    /// Integer-typed enum (coerced to string by the validator).
    #[tool(enum_values_priority = [1, 2, 3])]
    pub fn pick_priority(&self, priority: i32) -> String {
        format!("priority {}", priority)
    }

    /// Mix of string and integer values in the same enum (the
    /// attribute parser stores the token-stream text verbatim, so
    /// all values are stored as strings after the `to_token_stream`
    /// round-trip).
    #[tool(enum_values_mixed = ["a", "b", 1, 2])]
    pub fn pick_mixed(&self, mixed: String) -> String {
        format!("got {}", mixed)
    }
}

#[test]
fn enum_values_emits_json_schema_enum_array() {
    let defs = EnumValueTools::tool_definitions();
    let tool = defs.iter().find(|t| t.name == "pick_color").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let color_schema = &schema["properties"]["color"];
    let enum_arr = color_schema["enum"]
        .as_array()
        .expect("enum_values_* should emit a JSON Schema `enum` array");
    assert!(enum_arr.contains(&serde_json::json!("red")));
    assert!(enum_arr.contains(&serde_json::json!("green")));
    assert!(enum_arr.contains(&serde_json::json!("blue")));
}

#[test]
fn enum_values_integer_param_emits_schema_enum() {
    let defs = EnumValueTools::tool_definitions();
    let tool = defs.iter().find(|t| t.name == "pick_priority").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let priority_schema = &schema["properties"]["priority"];
    assert!(priority_schema.get("enum").is_some());
    let arr = priority_schema["enum"].as_array().unwrap();
    // Values are token-stream-rendered as `1`, `2`, `3` — confirm
    // the array contains exactly three entries.
    assert_eq!(arr.len(), 3);
}

#[test]
fn enum_values_mixed_emits_schema_enum() {
    let defs = EnumValueTools::tool_definitions();
    let tool = defs.iter().find(|t| t.name == "pick_mixed").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let mixed_schema = &schema["properties"]["mixed"];
    assert!(mixed_schema.get("enum").is_some());
    let arr = mixed_schema["enum"].as_array().unwrap();
    assert_eq!(arr.len(), 4);
}

#[test]
fn enum_values_method_still_callable() {
    // The attribute is purely declarative: the method body is still
    // invoked for any input. We confirm that a valid value flows
    // through end-to-end.
    let tools = EnumValueTools;
    let r = tools.call_tool("pick_color", &serde_json::json!({"color": "red"}));
    assert!(r.is_ok());
}
