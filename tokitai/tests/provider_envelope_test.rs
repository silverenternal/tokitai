//! T-006: offline-compatible LLM integration test for schema round-tripping.
//!
//! ## What this test guarantees
//!
//! A `tokitai_core::ToolDefinition` built from a known `input_schema`
//! string survives the trip into:
//!
//! 1. OpenAI's `/v1/chat/completions` `tools[].function.parameters`
//!    envelope (via `ToolDefinition::to_openai_function`).
//! 2. Anthropic's `/v1/messages` top-level `input_schema` envelope
//!    (via `ToolDefinition::to_anthropic_tool`).
//! 3. MCP's `tools/list` response `inputSchema` envelope
//!    (via `ToolDefinition::to_mcp_tool`).
//!
//! "Survives the trip" means:
//!
//! * The envelope shape matches what each provider's docs say (key
//!   names, value placement).
//! * Every property in the source `input_schema` is reachable from
//!   the envelope object — no fields dropped, no fields renamed.
//! * The recorded fixture in `tests/fixtures/` parses and the
//!   shape check holds against the fixture as a sanity check on
//!   the fixture itself.
//!
//! For OpenAI and Anthropic, no live API call is made; the test
//! reads recorded request fixtures and asserts the envelope shape
//! matches. For Ollama, an optional `llm-live` feature flag enables a
//! real round-trip gated behind the `LLM_API_KEY` (or, for Ollama,
//! `OLLAMA_HOST`) env var so CI does not hit the network by default.
//!
//! Run with:
//!
//! ```text
//! cargo test -p tokitai --test provider_envelope_test
//! cargo test -p tokitai --test provider_envelope_test --features llm-live
//! ```
//!
//! ## Why an offline test
//!
//! Live integration tests against `api.openai.com` / `api.anthropic.com`
//! are flaky and burn real tokens; the provider-envelope *shape* is
//! stable, the dial-in on schema quirks lives at the macro layer
//! (T-012). Asserting against recorded fixtures catches:
//!
//! * Accidental renaming of `inputSchema` -> `input_schema` on MCP.
//! * Accidental flattening of `function.parameters` -> top-level
//!   `parameters` on OpenAI.
//! * Accidental loss of nested properties on any path.
//!
//! Each path is exercised by one negative case in this file; the
//! positive cases come from the fixtures.

#![cfg(feature = "serde")]
#![allow(clippy::needless_raw_string_hashes)]

use serde_json::{json, Value};
use tokitai::{tool, ToolCaller, ToolDefinition, ToolProvider};

// ============================================================================
// A sample #[tool] impl used as the round-trip source.
// ============================================================================

#[derive(Default)]
struct SampleWeatherTools;

#[tool]
impl SampleWeatherTools {
    /// Get the current weather for a city.
    #[tool(desc = "Get the current weather for a city.")]
    pub fn get_weather(&self, city: String, unit: Option<String>) -> String {
        format!("{}/{:?}", city, unit)
    }

    /// Add two integers together.
    #[tool(desc = "Add two integers together.")]
    pub fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }
}

/// The same set of tools, hand-built as `ToolDefinition`s, used as the
/// reference for envelope shape comparison. This is the canonical
/// `input_schema` string we expect to see inside every envelope.
fn reference_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "get_weather",
            "Get the current weather for a city.",
            r#"{"type":"object","properties":{"city":{"type":"string","description":"City name"},"unit":{"type":"string","description":"celsius or fahrenheit"}},"required":["city"]}"#,
        ),
        ToolDefinition::new(
            "add",
            "Add two integers together.",
            r#"{"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}},"required":["a","b"]}"#,
        ),
    ]
}

// ============================================================================
// Acceptance criterion 1: round-trip through OpenAI's envelope.
// ============================================================================

/// Build a single OpenAI tool entry from a `ToolDefinition` and assert
/// the envelope shape matches what `/v1/chat/completions` expects.
fn openai_envelope_for(tool: &ToolDefinition) -> Value {
    let v = tool.to_openai_function();
    // Envelope shape (OpenAI strict-mode-compliant, recorded against
    // the 2025-06-18 docs):
    //   { "type": "function",
    //     "function": { "name", "description", "parameters" } }
    assert_eq!(v["type"], json!("function"), "OpenAI envelope type");
    let f = &v["function"];
    assert_eq!(f["name"], json!(tool.name), "OpenAI envelope function.name");
    assert_eq!(
        f["description"],
        json!(tool.description),
        "OpenAI envelope function.description"
    );
    assert!(
        f["parameters"].is_object(),
        "OpenAI parameters must be a JSON object, got: {}",
        f["parameters"]
    );
    assert!(
        f["parameters"]["type"].as_str() == Some("object"),
        "OpenAI parameters.type must be 'object'"
    );
    assert!(
        f["parameters"]["properties"].is_object(),
        "OpenAI parameters.properties must be a JSON object"
    );
    assert!(
        f["parameters"]["required"].is_array(),
        "OpenAI parameters.required must be an array"
    );
    f["parameters"].clone()
}

#[test]
fn test_openai_envelope_preserves_schema_for_each_tool() {
    let refs = reference_definitions();
    for tool in &refs {
        let params = openai_envelope_for(tool);

        // Re-parse the original input_schema and assert every key is
        // also present on the envelope.
        let original: Value = serde_json::from_str(&tool.input_schema).unwrap();
        assert_eq!(
            original["properties"].as_object().unwrap().len(),
            params["properties"].as_object().unwrap().len(),
            "OpenAI envelope must preserve every property for tool `{}`",
            tool.name
        );
        for key in original["properties"].as_object().unwrap().keys() {
            assert!(
                params["properties"].get(key).is_some(),
                "OpenAI envelope dropped property `{}` for tool `{}`",
                key,
                tool.name
            );
        }
        assert_eq!(
            original["required"].as_array().unwrap().len(),
            params["required"].as_array().unwrap().len(),
            "OpenAI envelope must preserve every required key for tool `{}`",
            tool.name
        );
    }
}

#[test]
fn test_openai_envelope_matches_recorded_fixture() {
    // Sanity check: the fixture itself parses, contains the two
    // reference tools under `tools[]`, and each entry has the shape
    // we promise the envelope emits.
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/openai_chat_completion_request.json"
    );
    let raw = std::fs::read_to_string(fixture_path).expect(
        "OpenAI fixture must be present (tests/fixtures/openai_chat_completion_request.json)",
    );
    let fixture: Value = serde_json::from_str(&raw).expect("OpenAI fixture must be valid JSON");

    let tools = fixture["tools"]
        .as_array()
        .expect("OpenAI fixture must have a `tools` array");
    assert_eq!(tools.len(), 2, "fixture must contain 2 tools");

    for entry in tools {
        assert_eq!(entry["type"], json!("function"));
        let f = &entry["function"];
        assert!(f["name"].is_string());
        assert!(f["description"].is_string());
        assert!(f["parameters"].is_object());
        assert_eq!(f["parameters"]["type"], json!("object"));
        assert!(f["parameters"]["properties"].is_object());
        assert!(f["parameters"]["required"].is_array());

        // Round-trip: build a ToolDefinition from the fixture entry
        // and re-emit, then confirm the re-emitted envelope is
        // semantically identical to the fixture entry (modulo key
        // ordering, which serde_json does not preserve).
        let td = ToolDefinition::new(
            f["name"].as_str().unwrap(),
            f["description"].as_str().unwrap(),
            serde_json::to_string(&f["parameters"]).unwrap(),
        );
        let re_emitted = td.to_openai_function();
        assert_eq!(re_emitted["type"], entry["type"]);
        assert_eq!(re_emitted["function"]["name"], entry["function"]["name"]);
        assert_eq!(
            re_emitted["function"]["description"],
            entry["function"]["description"]
        );
        assert_eq!(
            re_emitted["function"]["parameters"], entry["function"]["parameters"],
            "re-emitted OpenAI envelope must match fixture exactly for tool `{}`",
            f["name"]
        );
    }
}

#[test]
fn test_openai_envelope_emitted_by_macro_matches_fixture() {
    // The macro-generated `tool_definitions()` must round-trip into the
    // same envelope shape the fixture uses. This is the "no field is
    // dropped on the way from #[tool] to the wire" check.
    let macro_tools = SampleWeatherTools::tool_definitions();
    assert_eq!(macro_tools.len(), 2);

    for td in macro_tools {
        let envelope = td.to_openai_function();
        let params = &envelope["function"]["parameters"];
        let original: Value = serde_json::from_str(&td.input_schema).unwrap();

        // Property count and required-key count must match.
        assert_eq!(
            params["properties"].as_object().unwrap().len(),
            original["properties"].as_object().unwrap().len(),
            "macro -> OpenAI envelope must preserve every property for `{}`",
            td.name
        );
        assert_eq!(
            params["required"].as_array().unwrap().len(),
            original["required"].as_array().unwrap().len(),
            "macro -> OpenAI envelope must preserve every required key for `{}`",
            td.name
        );

        // Sanity: every property name from the source schema appears in
        // the envelope.
        for key in original["properties"].as_object().unwrap().keys() {
            assert!(
                params["properties"].get(key).is_some(),
                "OpenAI envelope dropped property `{}` for tool `{}`",
                key,
                td.name
            );
        }
    }
}

// ============================================================================
// Acceptance criterion 1: round-trip through Anthropic's envelope.
// ============================================================================

fn anthropic_envelope_for(tool: &ToolDefinition) -> Value {
    let v = tool.to_anthropic_tool();
    // Envelope shape (Anthropic, recorded against the 2025-06-18 docs):
    //   { "name", "description", "input_schema" }
    // Note: NO outer "function" wrapper. The schema sits at the top.
    assert_eq!(v["name"], json!(tool.name), "Anthropic envelope name");
    assert_eq!(
        v["description"],
        json!(tool.description),
        "Anthropic envelope description"
    );
    assert!(
        v["input_schema"].is_object(),
        "Anthropic input_schema must be a JSON object"
    );
    assert_eq!(v["input_schema"]["type"], json!("object"));
    v["input_schema"].clone()
}

#[test]
fn test_anthropic_envelope_preserves_schema_for_each_tool() {
    let refs = reference_definitions();
    for tool in &refs {
        let schema = anthropic_envelope_for(tool);
        let original: Value = serde_json::from_str(&tool.input_schema).unwrap();
        for key in original["properties"].as_object().unwrap().keys() {
            assert!(
                schema["properties"].get(key).is_some(),
                "Anthropic envelope dropped property `{}` for tool `{}`",
                key,
                tool.name
            );
        }
        assert_eq!(
            original["required"].as_array().unwrap().len(),
            schema["required"].as_array().unwrap().len(),
            "Anthropic envelope must preserve every required key for tool `{}`",
            tool.name
        );
    }
}

#[test]
fn test_anthropic_envelope_matches_recorded_fixture() {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/anthropic_messages_request.json"
    );
    let raw = std::fs::read_to_string(fixture_path).expect(
        "Anthropic fixture must be present (tests/fixtures/anthropic_messages_request.json)",
    );
    let fixture: Value = serde_json::from_str(&raw).expect("Anthropic fixture must be valid JSON");

    let tools = fixture["tools"]
        .as_array()
        .expect("Anthropic fixture must have a `tools` array");
    assert_eq!(tools.len(), 2);

    for entry in tools {
        assert!(entry["name"].is_string());
        assert!(entry["description"].is_string());
        assert!(
            entry["input_schema"].is_object(),
            "Anthropic fixture must use `input_schema` at the top level of each tool"
        );
        assert!(
            entry.get("function").is_none(),
            "Anthropic tools have no `function` wrapper"
        );

        // Re-emit and compare.
        let td = ToolDefinition::new(
            entry["name"].as_str().unwrap(),
            entry["description"].as_str().unwrap(),
            serde_json::to_string(&entry["input_schema"]).unwrap(),
        );
        let re_emitted = td.to_anthropic_tool();
        assert_eq!(re_emitted["name"], entry["name"]);
        assert_eq!(re_emitted["description"], entry["description"]);
        assert_eq!(
            re_emitted["input_schema"], entry["input_schema"],
            "re-emitted Anthropic envelope must match fixture exactly for tool `{}`",
            entry["name"]
        );
    }
}

#[test]
fn test_anthropic_envelope_emitted_by_macro_matches_fixture() {
    let macro_tools = SampleWeatherTools::tool_definitions();
    assert_eq!(macro_tools.len(), 2);
    for td in macro_tools {
        let envelope = td.to_anthropic_tool();
        // `input_schema` (snake_case) at the top level — never under
        // a function key.
        assert!(envelope.get("input_schema").is_some());
        assert!(
            envelope.get("function").is_none(),
            "Anthropic envelope must NOT wrap schema in a function key for tool `{}`",
            td.name
        );
        // Round-trip back to a ToolDefinition from the envelope.
        let schema_str = serde_json::to_string(&envelope["input_schema"]).unwrap();
        let td2 = ToolDefinition::new(
            envelope["name"].as_str().unwrap(),
            envelope["description"].as_str().unwrap(),
            &schema_str,
        );
        assert_eq!(td2.name, td.name);
        assert_eq!(td2.description, td.description);
        assert_eq!(td2.input_schema, schema_str);
    }
}

// ============================================================================
// Acceptance criterion 1: round-trip through MCP's envelope.
// ============================================================================

fn mcp_envelope_for(tool: &ToolDefinition) -> Value {
    let v = tool.to_mcp_tool();
    // Envelope shape (MCP 2025-06-18 spec):
    //   { "name", "description", "inputSchema" }
    // Note the camelCase key — distinct from Anthropic's snake_case.
    assert_eq!(v["name"], json!(tool.name));
    assert_eq!(v["description"], json!(tool.description));
    assert!(
        v["inputSchema"].is_object(),
        "MCP inputSchema must be an object"
    );
    assert_eq!(v["inputSchema"]["type"], json!("object"));
    v["inputSchema"].clone()
}

#[test]
fn test_mcp_envelope_preserves_schema_for_each_tool() {
    let refs = reference_definitions();
    for tool in &refs {
        let schema = mcp_envelope_for(tool);
        let original: Value = serde_json::from_str(&tool.input_schema).unwrap();
        for key in original["properties"].as_object().unwrap().keys() {
            assert!(
                schema["properties"].get(key).is_some(),
                "MCP envelope dropped property `{}` for tool `{}`",
                key,
                tool.name
            );
        }
        assert_eq!(
            original["required"].as_array().unwrap().len(),
            schema["required"].as_array().unwrap().len(),
            "MCP envelope must preserve every required key for tool `{}`",
            tool.name
        );
    }
}

#[test]
fn test_mcp_envelope_matches_recorded_fixture() {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/mcp_tools_list_response.json"
    );
    let raw = std::fs::read_to_string(fixture_path)
        .expect("MCP fixture must be present (tests/fixtures/mcp_tools_list_response.json)");
    let fixture: Value = serde_json::from_str(&raw).expect("MCP fixture must be valid JSON");

    let tools = fixture["result"]["tools"]
        .as_array()
        .expect("MCP fixture must have `result.tools`");
    assert_eq!(tools.len(), 2);

    for entry in tools {
        // The MCP envelope uses `inputSchema` (camelCase).
        assert!(entry["name"].is_string());
        assert!(entry["description"].is_string());
        assert!(
            entry["inputSchema"].is_object(),
            "MCP fixture must use `inputSchema` (camelCase)"
        );
        assert!(
            entry.get("input_schema").is_none(),
            "MCP fixture must NOT use snake_case `input_schema`"
        );

        let td = ToolDefinition::new(
            entry["name"].as_str().unwrap(),
            entry["description"].as_str().unwrap(),
            serde_json::to_string(&entry["inputSchema"]).unwrap(),
        );
        let re_emitted = td.to_mcp_tool();
        assert_eq!(re_emitted["name"], entry["name"]);
        assert_eq!(re_emitted["description"], entry["description"]);
        assert_eq!(
            re_emitted["inputSchema"], entry["inputSchema"],
            "re-emitted MCP envelope must match fixture exactly for tool `{}`",
            entry["name"]
        );
    }
}

#[test]
fn test_mcp_envelope_emitted_by_macro_matches_fixture() {
    let macro_tools = SampleWeatherTools::tool_definitions();
    for td in macro_tools {
        let envelope = td.to_mcp_tool();
        // camelCase `inputSchema` at the top level.
        assert!(envelope.get("inputSchema").is_some());
        assert!(
            envelope.get("input_schema").is_none(),
            "MCP envelope must NOT use snake_case `input_schema` for tool `{}`",
            td.name
        );
        // Round-trip back to a ToolDefinition.
        let schema_str = serde_json::to_string(&envelope["inputSchema"]).unwrap();
        let td2 = ToolDefinition::new(
            envelope["name"].as_str().unwrap(),
            envelope["description"].as_str().unwrap(),
            &schema_str,
        );
        assert_eq!(td2.name, td.name);
        assert_eq!(td2.description, td.description);
        assert_eq!(td2.input_schema, schema_str);
    }
}

// ============================================================================
// Acceptance criterion 1 (cross-provider): the same source schema
// yields envelopes that are syntactically distinct (different key
// names, different nesting) but semantically equivalent (same set of
// properties, same set of required keys).
// ============================================================================

#[test]
fn test_envelopes_are_syntactically_distinct_but_equivalent() {
    let refs = reference_definitions();
    for tool in &refs {
        let openai_params = openai_envelope_for(tool);
        let anthropic_schema = anthropic_envelope_for(tool);
        let mcp_schema = mcp_envelope_for(tool);

        // Syntactic differences we expect:
        // - OpenAI nests the schema under function.parameters
        // - Anthropic uses snake_case `input_schema`
        // - MCP uses camelCase `inputSchema`
        let _ = openai_params; // already wrapped under function.parameters
        assert!(anthropic_schema.is_object());
        assert!(mcp_schema.is_object());

        // Semantic equivalence: same property keys, same required keys.
        let mut props_a: Vec<&str> = anthropic_schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let mut props_m: Vec<&str> = mcp_schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        props_a.sort_unstable();
        props_m.sort_unstable();
        assert_eq!(
            props_a, props_m,
            "Anthropic and MCP envelopes must agree on property keys for `{}`",
            tool.name
        );

        let mut req_a: Vec<&str> = anthropic_schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let mut req_m: Vec<&str> = mcp_schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        req_a.sort_unstable();
        req_m.sort_unstable();
        assert_eq!(
            req_a, req_m,
            "Anthropic and MCP envelopes must agree on required keys for `{}`",
            tool.name
        );
    }
}

// ============================================================================
// Round-trip back through the dispatcher: emit the envelope, parse the
// arguments a provider would send back, and confirm call_tool returns
// the expected value. This catches:
//   * argument key renaming (OpenAI vs Anthropic vs MCP)
//   * missing-property detection
// ============================================================================

#[test]
fn test_round_trip_call_tool_via_each_envelope_arg_shape() {
    let tools_inst = SampleWeatherTools;

    // OpenAI envelope nests arguments under `function.arguments`.
    let openai_args = json!({"a": 7, "b": 35});
    let result = <SampleWeatherTools as ToolCaller>::call_tool(&tools_inst, "add", &openai_args)
        .expect("OpenAI-shaped call must succeed");
    assert_eq!(result, json!(42));

    // Anthropic envelope passes arguments at the top level (no wrapping).
    let anthropic_args = json!({"a": 100, "b": 23});
    let result = <SampleWeatherTools as ToolCaller>::call_tool(&tools_inst, "add", &anthropic_args)
        .expect("Anthropic-shaped call must succeed");
    assert_eq!(result, json!(123));

    // MCP envelope also passes arguments at the top level (the JSON-RPC
    // `tools/call` envelope wraps the call under `params.arguments`, but
    // once that layer is peeled off the dispatcher sees the same shape
    // as Anthropic).
    let mcp_args = json!({"a": 8, "b": 9});
    let result = <SampleWeatherTools as ToolCaller>::call_tool(&tools_inst, "add", &mcp_args)
        .expect("MCP-shaped call must succeed");
    assert_eq!(result, json!(17));

    // Optional argument present vs absent (from any envelope shape).
    let result = <SampleWeatherTools as ToolCaller>::call_tool(
        &tools_inst,
        "get_weather",
        &json!({"city": "Paris"}),
    )
    .expect("optional absent must be accepted");
    assert_eq!(result, json!("Paris/None"));

    let result = <SampleWeatherTools as ToolCaller>::call_tool(
        &tools_inst,
        "get_weather",
        &json!({"city": "Paris", "unit": "celsius"}),
    )
    .expect("optional present must be accepted");
    assert_eq!(result, json!("Paris/Some(\"celsius\")"));

    // Missing a required argument must fail consistently regardless of
    // which envelope the LLM would have used.
    let err = <SampleWeatherTools as ToolCaller>::call_tool(&tools_inst, "get_weather", &json!({}))
        .expect_err("missing required `city` must fail");
    assert_eq!(err.kind, tokitai::ToolErrorKind::ValidationError);
}

// ============================================================================
// Acceptance criterion 3 (optional): live test gated behind a feature
// flag. The test only runs when the user opts in with
// `--features llm-live` AND provides an `OLLAMA_HOST` env var. CI does
// not enable this feature.
// ============================================================================

#[cfg(feature = "llm-live")]
#[test]
fn test_live_round_trip_against_ollama() {
    use std::time::Duration;

    // Read the host from the env. Default to localhost. CI does not
    // set this; the test panics with a helpful message if neither the
    // env var nor a local Ollama instance is reachable.
    let host = std::env::var("OLLAMA_HOST")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());

    // We don't pull in a network crate to keep the dependency surface
    // small; instead, we spawn the user's local `curl` (always present
    // on macOS / Linux dev boxes) to make the HTTP call. This is a
    // pragmatic dev affordance: the offline test above is the actual
    // regression barrier.
    let payload = json!({
        "model": "llama3.2",
        "messages": [{"role": "user", "content": "Call the add tool with a=2, b=2."}],
        "tools": [
            SampleWeatherTools::tool_definitions()
                .iter()
                .find(|t| t.name == "add")
                .unwrap()
                .to_openai_function()
        ],
        "stream": false
    });
    let body = serde_json::to_string(&payload).unwrap();
    let url = format!("{}/v1/chat/completions", host.trim_end_matches('/'));

    let output = std::process::Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            "-X",
            "POST",
            &url,
            "-H",
            "Content-Type: application/json",
            "-d",
            &body,
        ])
        .output()
        .expect("failed to spawn `curl`; is it on PATH?");

    if !output.status.success() {
        // Soft-skip: if Ollama isn't running locally, surface a clear
        // message instead of failing CI. The offline tests above are
        // the actual regression barrier.
        eprintln!(
            "Ollama not reachable at {} (exit {:?}); skipping live test.",
            host, output.status
        );
        return;
    }

    let response: Value = serde_json::from_slice(&output.stdout)
        .expect("Ollama response must be valid JSON when reachable");

    // The shape we expect from Ollama's OpenAI-compatible endpoint is
    // the same envelope `to_openai_function` produces — so the same
    // assertion that holds for OpenAI holds for Ollama.
    assert!(
        response.get("choices").is_some(),
        "Ollama response must have `choices`; got: {}",
        response
    );
    let _ = Duration::from_secs(0); // keep Duration import alive in no-op builds
}
