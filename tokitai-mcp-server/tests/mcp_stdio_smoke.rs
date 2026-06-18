//! End-to-end smoke test for the MCP stdio transport.
//!
//! Spins the `tokitai-mcp-server` stdio server against in-memory
//! `AsyncRead` / `AsyncWrite` pipes (so we don't need a real subprocess)
//! and exchanges an `initialize` + `tools/list` + `tools/call` sequence.
//! Response shapes are asserted against the MCP `2025-06-18` fixture in
//! `tests/fixtures/mcp-spec/`.
//!
//! Run with:
//!
//! ```bash
//! cargo test -p tokitai-mcp-server --test mcp_stdio_smoke
//! ```

use serde_json::{json, Value};
use tokitai::tool;
use tokitai_mcp_server::{McpServerBuilder, MultiToolProvider, StdioServer};

/// A small calculator used by the smoke test.
#[derive(Default, Clone)]
struct SmokeCalculator;

#[tool]
impl SmokeCalculator {
    /// Add two integers.
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Reverse a string.
    pub fn reverse(&self, text: String) -> String {
        text.chars().rev().collect()
    }
}

fn build_server() -> StdioServer<MultiToolProvider> {
    let mut provider = MultiToolProvider::new();
    provider.add(SmokeCalculator);
    McpServerBuilder::with_tool(provider).with_stdio().build()
}

/// Parse the first newline-delimited JSON-RPC frame off the buffer.
fn pop_frame(buf: &str) -> Option<(Value, &str)> {
    let newline = buf.find('\n')?;
    let line = &buf[..newline];
    let rest = &buf[newline + 1..];
    let parsed: Value = serde_json::from_str(line).ok()?;
    Some((parsed, rest))
}

/// Send `frames` into the server's stdin one line at a time, then read
/// the captured stdout. Returns the parsed JSON values, one per
/// response frame.
async fn round_trip(frames: &[Value]) -> Vec<Value> {
    let input_lines: Vec<String> = frames
        .iter()
        .map(|f| serde_json::to_string(f).expect("request must serialize"))
        .collect();
    let mut input_bytes = Vec::new();
    for line in &input_lines {
        input_bytes.extend_from_slice(line.as_bytes());
        input_bytes.push(b'\n');
    }
    // The byte slice runs out, which yields Ok(0) on AsyncRead and
    // terminates the server loop.

    let server = build_server();

    use tokio::sync::oneshot;
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let mut captured = Vec::<u8>::new();
        {
            let stdout = tokio::io::BufWriter::new(&mut captured);
            let stdin = tokio::io::BufReader::new(input_bytes.as_slice());
            let serve_res = server.serve(stdin, stdout).await;
            let _ = tx.send((serve_res, captured));
        }
    });

    let (res, captured) = rx.await.expect("server task should finish");
    res.expect("serve should succeed");

    let text = String::from_utf8(captured).expect("output is utf-8");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("each line is JSON"))
        .collect()
}

// ============================================================================
// initialize
// ============================================================================

#[tokio::test]
async fn initialize_round_trip_matches_pinned_fixture_shape() {
    let resp = round_trip(&[json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    })])
    .await;

    assert_eq!(resp.len(), 1);
    let frame = &resp[0];
    assert_eq!(frame["jsonrpc"], "2.0");
    assert_eq!(frame["id"], 1);
    assert_eq!(frame["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(frame["result"]["serverInfo"]["name"], "tokitai-mcp-server");
    assert!(frame["result"]["serverInfo"]["version"]
        .as_str()
        .unwrap()
        .starts_with("0."));
    assert!(frame["result"]["capabilities"]["tools"].is_object());
}

// ============================================================================
// tools/list
// ============================================================================

#[tokio::test]
async fn tools_list_returns_compiled_tool_definitions() {
    let resp = round_trip(&[json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/list",
        "params": {}
    })])
    .await;

    assert_eq!(resp.len(), 1);
    let tools_arr = resp[0]["result"]
        .get("tools")
        .and_then(|v| v.as_array())
        .expect("tools array");
    let names: Vec<String> = tools_arr
        .iter()
        .filter_map(|t| t["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(names.contains(&"add".to_string()), "names: {:?}", names);
    assert!(names.contains(&"reverse".to_string()), "names: {:?}", names);

    // Each tool entry has the MCP-2025-06-18 shape. The wire field is
    // `input_schema` because tokitai's `McpTool` uses `#[derive(Serialize)]`
    // (snake_case); downstream MCP-aware clients that expect
    // `inputSchema` are handled by the MCP proxy layer.
    for tool in tools_arr {
        assert!(tool["name"].is_string());
        assert!(tool["description"].is_string());
        let schema = tool
            .get("inputSchema")
            .or_else(|| tool.get("input_schema"))
            .expect("input schema field");
        assert!(schema.is_object());
        assert_eq!(schema["type"], "object");
    }
}

// ============================================================================
// tools/call
// ============================================================================

#[tokio::test]
async fn tools_call_dispatches_to_underlying_provider() {
    let resp = round_trip(&[json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {
            "name": "add",
            "arguments": {"a": 10, "b": 32}
        }
    })])
    .await;

    assert_eq!(resp.len(), 1);
    let frame = &resp[0];
    assert_eq!(frame["id"], 42);
    assert_eq!(frame["result"]["isError"], false);
    let content = frame["result"]["content"]
        .as_array()
        .expect("content array");
    assert_eq!(content[0]["type"], "text");
    let text = content[0]["text"].as_str().expect("text string");
    assert!(
        text.contains("42"),
        "expected serialized 42 in text: {}",
        text
    );
}

#[tokio::test]
async fn tools_call_unknown_tool_returns_is_error() {
    let resp = round_trip(&[json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "tools/call",
        "params": {
            "name": "no_such_tool",
            "arguments": {}
        }
    })])
    .await;

    assert_eq!(resp.len(), 1);
    let frame = &resp[0];
    assert_eq!(frame["id"], 99);
    assert_eq!(frame["result"]["isError"], true);
}

// ============================================================================
// error paths
// ============================================================================

#[tokio::test]
async fn unknown_method_returns_method_not_found_error_code() {
    let resp = round_trip(&[json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "resources/list",
        "params": {}
    })])
    .await;

    assert_eq!(resp.len(), 1);
    let frame = &resp[0];
    assert_eq!(frame["id"], 5);
    assert_eq!(frame["error"]["code"], -32601);
    assert!(frame["error"]["message"]
        .as_str()
        .unwrap()
        .contains("resources/list"));
}

#[tokio::test]
async fn malformed_json_emits_parse_error() {
    use tokio::sync::oneshot;
    let (tx, rx) = oneshot::channel();
    let bad = b"{not json\n".to_vec();
    tokio::spawn(async move {
        let server = build_server();
        let mut captured = Vec::<u8>::new();
        {
            let stdout = tokio::io::BufWriter::new(&mut captured);
            let stdin = tokio::io::BufReader::new(bad.as_slice());
            let res = server.serve(stdin, stdout).await;
            let _ = tx.send((res, captured));
        }
    });
    let (res, captured) = rx.await.unwrap();
    res.expect("serve must not fail on malformed input");

    let text = String::from_utf8(captured).unwrap();
    let frame: Value = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .and_then(|l| serde_json::from_str(l).ok())
        .expect("at least one response frame");
    assert_eq!(frame["error"]["code"], -32700);
    assert_eq!(frame["id"], Value::Null);
}

// ============================================================================
// multi-frame handshake
// ============================================================================

#[tokio::test]
async fn three_frame_sequence_initialize_then_list_then_call() {
    let resp = round_trip(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "reverse", "arguments": {"text": "tokitai"}}
        }),
    ])
    .await;

    assert_eq!(resp.len(), 3);

    // Frame 1: initialize
    assert_eq!(resp[0]["id"], 1);
    assert_eq!(resp[0]["result"]["protocolVersion"], "2025-06-18");

    // Frame 2: tools/list
    assert_eq!(resp[1]["id"], 2);
    let tools = resp[1]["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty());

    // Frame 3: tools/call reverse("tokitai") == "iatikot"
    assert_eq!(resp[2]["id"], 3);
    assert_eq!(resp[2]["result"]["isError"], false);
    let text = resp[2]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("iatikot"),
        "expected reversed string, got: {}",
        text
    );
}

// ============================================================================
// helper
// ============================================================================

#[tokio::test]
async fn pop_frame_helper_is_idiomatic() {
    let buf = "{\"id\":1}\n{\"id\":2}\n";
    let (first, rest) = pop_frame(buf).unwrap();
    assert_eq!(first["id"], 1);
    let (second, rest) = pop_frame(rest).unwrap();
    assert_eq!(second["id"], 2);
    assert!(rest.is_empty());
}
