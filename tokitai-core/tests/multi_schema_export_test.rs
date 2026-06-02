//! Integration tests for the multi-format tool exporters on
//! [`ToolDefinition`]: `to_openai_function`, `to_anthropic_tool`, and
//! `to_mcp_tool`.
//!
//! Run with: `cargo test -p tokitai-core --test multi_schema_export_test`

#![cfg(feature = "serde")]

use serde_json::json;
use tokitai_core::ToolDefinition;

/// Build a representative `ToolDefinition` for tests. The schema contains
/// two parameters of different types so the inner JSON Schema is non-trivial
/// and easy to compare across exporters.
fn sample_tool() -> ToolDefinition {
    let schema = json!({
        "type": "object",
        "properties": {
            "city": {
                "type": "string",
                "description": "City name to look up",
            },
            "units": {
                "type": "string",
                "enum": ["celsius", "fahrenheit"],
                "description": "Temperature unit",
            }
        },
        "required": ["city"],
    });
    ToolDefinition::new(
        "get_weather",
        "Get the current weather for a city",
        serde_json::to_string(&schema).unwrap(),
    )
}

#[test]
fn test_to_openai_function_shape() {
    let td = sample_tool();
    let v = td.to_openai_function();

    // Top-level: {"type": "function", "function": {...}}
    assert_eq!(v["type"], json!("function"));

    let function = &v["function"];
    assert!(function.is_object(), "function must be an object");
    assert_eq!(function["name"], json!("get_weather"));
    assert_eq!(
        function["description"],
        json!("Get the current weather for a city")
    );

    // `parameters` should be the full JSON Schema (object type, properties
    // and required array preserved verbatim).
    let params = &function["parameters"];
    assert_eq!(params["type"], json!("object"));
    assert_eq!(params["properties"]["city"]["type"], json!("string"));
    assert_eq!(params["properties"]["units"]["type"], json!("string"));
    assert_eq!(params["required"], json!(["city"]));
}

#[test]
fn test_to_anthropic_tool_shape() {
    let td = sample_tool();
    let v = td.to_anthropic_tool();

    // Top-level keys: name, description, input_schema (snake_case).
    assert_eq!(v["name"], json!("get_weather"));
    assert_eq!(
        v["description"],
        json!("Get the current weather for a city")
    );
    assert!(v.get("input_schema").is_some(), "must have input_schema");
    assert!(
        v.get("inputSchema").is_none(),
        "Anthropic uses snake_case input_schema, not camelCase"
    );
    assert!(
        v.get("type").is_none(),
        "Anthropic tool definitions do not use a top-level 'type' field"
    );
    assert!(
        v.get("function").is_none(),
        "Anthropic tool definitions do not wrap fields in 'function'"
    );

    let input_schema = &v["input_schema"];
    assert_eq!(input_schema["type"], json!("object"));
    assert_eq!(input_schema["properties"]["city"]["type"], json!("string"));
    assert_eq!(input_schema["required"], json!(["city"]));
}

#[test]
fn test_to_mcp_tool_shape() {
    let td = sample_tool();
    let v = td.to_mcp_tool();

    // Top-level keys: name, description, inputSchema (camelCase).
    assert_eq!(v["name"], json!("get_weather"));
    assert_eq!(
        v["description"],
        json!("Get the current weather for a city")
    );
    assert!(v.get("inputSchema").is_some(), "must have inputSchema");
    assert!(
        v.get("input_schema").is_none(),
        "MCP uses camelCase inputSchema, not snake_case"
    );
    assert!(
        v.get("type").is_none(),
        "MCP tool definitions do not use a top-level 'type' field"
    );
    assert!(
        v.get("function").is_none(),
        "MCP tool definitions do not wrap fields in 'function'"
    );

    let input_schema = &v["inputSchema"];
    assert_eq!(input_schema["type"], json!("object"));
    assert_eq!(input_schema["properties"]["city"]["type"], json!("string"));
    assert_eq!(input_schema["required"], json!(["city"]));
}

#[test]
fn test_all_three_have_same_parameters() {
    let td = sample_tool();
    let openai = td.to_openai_function();
    let anthropic = td.to_anthropic_tool();
    let mcp = td.to_mcp_tool();

    // Extract the inner JSON Schema from each envelope.
    let openai_params = &openai["function"]["parameters"];
    let anthropic_params = &anthropic["input_schema"];
    let mcp_params = &mcp["inputSchema"];

    // All three protocols receive the exact same schema value.
    assert_eq!(
        openai_params, anthropic_params,
        "OpenAI and Anthropic must receive identical parameters"
    );
    assert_eq!(
        anthropic_params, mcp_params,
        "Anthropic and MCP must receive identical parameters"
    );
    assert_eq!(
        openai_params, mcp_params,
        "OpenAI and MCP must receive identical parameters"
    );

    // Sanity: the schema body really is the JSON Schema we constructed
    // (not a stringified version, not an empty object).
    assert_eq!(openai_params["type"], json!("object"));
    assert!(openai_params["properties"].is_object());
    assert_eq!(openai_params["required"], json!(["city"]));
}

#[test]
fn test_openai_envelope_keys() {
    // Enforce the exact outer key set for the OpenAI shape: it must
    // contain exactly `type` and `function` and nothing else.
    let td = sample_tool();
    let v = td.to_openai_function();

    let obj = v.as_object().expect("must be a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["function", "type"]);
}

#[test]
fn test_anthropic_envelope_keys() {
    // Enforce the exact outer key set for the Anthropic shape.
    let td = sample_tool();
    let v = td.to_anthropic_tool();

    let obj = v.as_object().expect("must be a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["description", "input_schema", "name"]);
}

#[test]
fn test_mcp_envelope_keys() {
    // Enforce the exact outer key set for the MCP shape.
    let td = sample_tool();
    let v = td.to_mcp_tool();

    let obj = v.as_object().expect("must be a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["description", "inputSchema", "name"]);
}

#[test]
fn test_invalid_input_schema_still_produces_valid_envelope() {
    // If the stored input_schema is not valid JSON, the exporters should
    // still return a structurally valid envelope (with an empty object
    // fallback for the inner schema) rather than panic.
    let td = ToolDefinition::new("broken", "Tool with invalid schema", "{ not valid json");

    let openai = td.to_openai_function();
    assert_eq!(openai["type"], json!("function"));
    assert_eq!(openai["function"]["name"], json!("broken"));
    assert_eq!(
        openai["function"]["parameters"],
        json!({}),
        "invalid schema must fall back to an empty object"
    );

    let anthropic = td.to_anthropic_tool();
    assert_eq!(anthropic["name"], json!("broken"));
    assert_eq!(anthropic["input_schema"], json!({}));

    let mcp = td.to_mcp_tool();
    assert_eq!(mcp["name"], json!("broken"));
    assert_eq!(mcp["inputSchema"], json!({}));
}
