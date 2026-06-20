//! T-034: Ollama provider.
//!
//! Wire format: <https://github.com/ollama/ollama/blob/main/docs/api.md#chat-request-with-tools>.
//! The endpoint is `/api/chat` and the request/response shape is
//! near-identical to OpenAI's. The local install is assumed at
//! `http://localhost:11434` and is reached with no auth header.
//!
//! T-043: the request is now an `InferenceRequest`. Ollama's
//! `/api/chat` API natively accepts `system` (as a message role
//! OR a `system` field on the request), `temperature`, `stop`,
//! `seed`, and `format` (its `response_format` equivalent). The
//! remaining T-043 knobs (`tool_choice`, `parallel_tool_calls`,
//! `stream`) are forwarded as best as the wire format allows:
//! `tool_choice` is mapped to Ollama's "let the model pick"
//! (no native equivalent), `parallel_tool_calls` is dropped, and
//! `stream` is rendered as the `stream` body field.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokitai_core::ToolDefinition;

use super::{
    ChatMessage, CompletionResponse, InferenceRequest, Provider, ProviderToolCall, ResponseFormat,
};

/// Default base URL for the local Ollama install.
pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";

/// Configuration for the Ollama provider.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// Base URL without trailing slash.
    pub base_url: String,
    /// Model name (e.g. `llama3.1`).
    pub model: String,
}

impl OllamaConfig {
    /// Build a config from CLI args.
    pub fn from_args(base_url: Option<String>, model: String) -> Self {
        Self {
            base_url: base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            model,
        }
    }
}

/// Ollama `/api/chat` provider.
pub struct OllamaProvider {
    config: OllamaConfig,
    client: Client,
    /// Pre-computed chat-completions URL. Computed once in `new()`
    /// so `complete_with_tools` does not pay the `format!` cost
    /// on every call.
    api_url: String,
    /// Pre-rendered tool envelopes. `complete_with_tools` clones
    /// this vector when building the request body, so a populated
    /// cache keeps the hot path allocation-free. Callers should
    /// refresh the cache via [`Self::set_tools_cache`] whenever
    /// the active tool set changes (e.g. before each chat turn).
    tools_cache: Vec<Value>,
}

impl OllamaProvider {
    /// Build a new Ollama provider.
    pub fn new(config: OllamaConfig) -> Self {
        let api_url = format!("{}/api/chat", config.base_url);
        Self {
            config,
            client: Client::new(),
            api_url,
            tools_cache: Vec::new(),
        }
    }

    /// Replace the cached tool envelopes with pre-rendered
    /// `OpenAI` envelopes. Call once per tool-set change to keep
    /// `complete_with_tools` off the per-call render path.
    pub fn set_tools_cache(&mut self, tools: &[ToolDefinition]) {
        self.tools_cache = tools
            .iter()
            .map(|t| t.to_openai_function()["function"].clone())
            .collect();
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    async fn complete_with_tools(
        &self,
        req: &InferenceRequest,
    ) -> anyhow::Result<CompletionResponse> {
        let body = self.build_wire_body(req);

        let resp = self
            .client
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        let parsed: OllamaResponse = resp.error_for_status()?.json().await?;
        Ok(parsed.into_completion())
    }
}

impl OllamaProvider {
    /// Build the JSON body for `/api/chat`. Extracted from
    /// `complete_with_tools` so the wire shape is testable
    /// without a mock reqwest. T-043 wires the sampling knobs,
    /// T-049 wires `response_format` onto the `format` field.
    fn build_wire_body(&self, req: &InferenceRequest) -> Value {
        // T-043: Ollama's wire format does not have a dedicated
        // `system` field; it accepts a `system`-role message
        // instead. Forward the system prompt as the first message
        // so the model sees it (was silently dropped before T-043
        // — see T-038 Bug 4).
        let mut wire_messages: Vec<Value> = Vec::with_capacity(req.messages.len() + 1);
        if let Some(sys) = req.system.as_deref() {
            wire_messages.push(json!({"role": "system", "content": sys}));
        }
        for m in &req.messages {
            wire_messages.push(ollama_message(m));
        }

        // Prefer the pre-rendered cache when it matches the active
        // tool slice (length-equality is the cheap approximation);
        // fall back to rendering on demand otherwise.
        let wire_tools: Vec<Value> =
            if self.tools_cache.len() == req.tools.len() && !req.tools.is_empty() {
                self.tools_cache.clone()
            } else {
                req.tools
                    .iter()
                    .map(|t| t.to_openai_function()["function"].clone())
                    .collect()
            };

        // T-043: every optional knob is rendered only when the
        // caller set it. `stream: false` is always present (Ollama
        // defaults to streaming otherwise).
        let mut body = json!({
            "model": self.config.model,
            "messages": wire_messages,
            "stream": req.stream,
        });
        if !wire_tools.is_empty() {
            body["tools"] = json!(wire_tools);
        }
        if let Some(opts) = ollama_options(req) {
            body["options"] = opts;
        }
        if let Some(rf) = req.response_format.as_ref() {
            body["format"] = json!(ollama_response_format(rf));
        }
        body
    }
}

// -- internal helpers ---------------------------------------------------

/// Map the T-043 sampling knobs onto Ollama's `options` block.
/// `tool_choice`, `parallel_tool_calls`, and `max_tokens` have no
/// direct Ollama equivalent and are dropped; `temperature`, `stop`,
/// and `seed` are forwarded.
fn ollama_options(req: &InferenceRequest) -> Option<Value> {
    let mut opts = serde_json::Map::new();
    if let Some(temp) = req.temperature {
        opts.insert("temperature".into(), json!(temp));
    }
    if let Some(stop) = req.stop.as_ref() {
        opts.insert("stop".into(), json!(stop));
    }
    if let Some(seed) = req.seed {
        opts.insert("seed".into(), json!(seed));
    }
    if opts.is_empty() {
        None
    } else {
        Some(Value::Object(opts))
    }
}

/// Map `ResponseFormat` to Ollama's `format` field. Ollama accepts
/// either the string `"json"` (free-form JSON object) or a JSON
/// schema object. The `JsonSchema` value is forwarded verbatim so
/// callers can pin the exact shape.
fn ollama_response_format(rf: &ResponseFormat) -> Value {
    match rf {
        ResponseFormat::JsonObject => json!("json"),
        ResponseFormat::JsonSchema(v) => v.clone(),
    }
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    #[serde(default)]
    message: Option<OllamaAssistant>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaAssistant {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Debug, Deserialize)]
struct OllamaToolCall {
    function: OllamaToolCallFn,
}

#[derive(Debug, Deserialize)]
struct OllamaToolCallFn {
    name: String,
    /// Arguments as a structured object (Ollama parses it for us).
    arguments: Value,
}

impl OllamaResponse {
    fn into_completion(self) -> CompletionResponse {
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        if let Some(m) = self.message {
            content = m.content;
            tool_calls = m
                .tool_calls
                .into_iter()
                .enumerate()
                .map(|(idx, tc)| ProviderToolCall {
                    // Ollama does not always assign a tool-call id,
                    // so we synthesise a stable one from the call
                    // index. The cache key is derived from the
                    // model + messages + envelopes, not from this
                    // id, so stability across runs is not required.
                    id: format!("ollama_call_{idx}"),
                    name: tc.function.name,
                    arguments: tc.function.arguments,
                })
                .collect();
        }
        let usage = match (self.prompt_eval_count, self.eval_count) {
            (Some(p), Some(c)) => Some(super::UsageReport {
                prompt_tokens: p,
                completion_tokens: c,
                total_tokens: p + c,
            }),
            _ => None,
        };
        CompletionResponse {
            content,
            tool_calls,
            usage,
        }
    }
}

fn ollama_message(m: &ChatMessage) -> Value {
    match m {
        ChatMessage::User { content } => json!({"role": "user", "content": content}),
        ChatMessage::Assistant {
            content,
            tool_calls,
        } => {
            let tcs: Vec<Value> = tool_calls
                .iter()
                .map(|tc| {
                    json!({
                        "function": {
                            "name": tc.name,
                            "arguments": tc.arguments,
                        }
                    })
                })
                .collect();
            json!({
                "role": "assistant",
                "content": content,
                "tool_calls": tcs,
            })
        }
        ChatMessage::Tool {
            tool_call_id,
            content,
        } => {
            json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": content,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ProviderToolCall, ResponseFormat};
    use serde_json::json;

    #[test]
    fn ollama_message_user() {
        let m = ChatMessage::User {
            content: "hi".into(),
        };
        let v = ollama_message(&m);
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "hi");
    }

    #[test]
    fn ollama_message_assistant_with_tool_calls() {
        let m = ChatMessage::Assistant {
            content: Some("thinking...".into()),
            tool_calls: vec![ProviderToolCall {
                id: "c1".into(),
                name: "echo".into(),
                arguments: json!({"x": 1}),
            }],
        };
        let v = ollama_message(&m);
        assert_eq!(v["role"], "assistant");
        assert_eq!(v["content"], "thinking...");
        assert_eq!(v["tool_calls"][0]["function"]["name"], "echo");
        assert_eq!(v["tool_calls"][0]["function"]["arguments"]["x"], 1);
    }

    #[test]
    fn ollama_message_tool() {
        let m = ChatMessage::Tool {
            tool_call_id: "call_42".into(),
            content: "result".into(),
        };
        let v = ollama_message(&m);
        assert_eq!(v["role"], "tool");
        assert_eq!(v["tool_call_id"], "call_42");
        assert_eq!(v["content"], "result");
    }

    #[test]
    fn ollama_response_into_completion_text_only() {
        let resp = OllamaResponse {
            message: Some(OllamaAssistant {
                content: "hello world".into(),
                tool_calls: vec![],
            }),
            prompt_eval_count: Some(5),
            eval_count: Some(3),
        };
        let c = resp.into_completion();
        assert_eq!(c.content, "hello world");
        assert!(c.tool_calls.is_empty());
        let usage = c.usage.expect("usage should be present");
        assert_eq!(usage.prompt_tokens, 5);
        assert_eq!(usage.completion_tokens, 3);
        assert_eq!(usage.total_tokens, 8);
    }

    #[test]
    fn ollama_response_into_completion_tool_calls() {
        let resp = OllamaResponse {
            message: Some(OllamaAssistant {
                content: String::new(),
                tool_calls: vec![
                    OllamaToolCall {
                        function: OllamaToolCallFn {
                            name: "echo".into(),
                            arguments: json!({"k": "v"}),
                        },
                    },
                    OllamaToolCall {
                        function: OllamaToolCallFn {
                            name: "other".into(),
                            arguments: json!({}),
                        },
                    },
                ],
            }),
            prompt_eval_count: None,
            eval_count: None,
        };
        let c = resp.into_completion();
        assert_eq!(c.tool_calls.len(), 2);
        assert_eq!(c.tool_calls[0].name, "echo");
        assert_eq!(c.tool_calls[0].id, "ollama_call_0");
        assert_eq!(c.tool_calls[0].arguments, json!({"k": "v"}));
        assert_eq!(c.tool_calls[1].name, "other");
        assert_eq!(c.tool_calls[1].id, "ollama_call_1");
        assert!(c.usage.is_none());
    }

    #[test]
    fn ollama_response_into_completion_empty_message() {
        let resp = OllamaResponse {
            message: None,
            prompt_eval_count: None,
            eval_count: None,
        };
        let c = resp.into_completion();
        assert_eq!(c.content, "");
        assert!(c.tool_calls.is_empty());
        assert!(c.usage.is_none());
    }

    #[test]
    fn ollama_config_from_args_strips_trailing_slash() {
        let cfg = OllamaConfig::from_args(Some("http://localhost:11434/".into()), "m".into());
        assert_eq!(cfg.base_url, "http://localhost:11434");
        assert_eq!(cfg.model, "m");
    }

    #[test]
    fn ollama_provider_name() {
        let p = OllamaProvider::new(OllamaConfig::from_args(None, "m".into()));
        assert_eq!(p.name(), "ollama");
    }

    #[test]
    fn ollama_options_only_includes_set_knobs() {
        let req = InferenceRequest::new("hi", vec![]);
        assert!(ollama_options(&req).is_none());

        let req = InferenceRequest::new("hi", vec![]).with_temperature(0.5);
        let opts = ollama_options(&req).unwrap();
        assert_eq!(opts["temperature"], 0.5);

        let req = InferenceRequest::new("hi", vec![])
            .with_stop(vec!["END".into()])
            .with_seed(7);
        let opts = ollama_options(&req).unwrap();
        assert_eq!(opts["stop"][0], "END");
        assert_eq!(opts["seed"], 7);
    }

    #[test]
    fn ollama_response_format_mapping() {
        // JsonObject -> "json" string.
        let v = ollama_response_format(&ResponseFormat::JsonObject);
        assert_eq!(v, json!("json"));

        // JsonSchema -> the schema value verbatim.
        let schema = json!({"type": "object", "properties": {"x": {"type": "number"}}});
        let v = ollama_response_format(&ResponseFormat::JsonSchema(schema.clone()));
        assert_eq!(v, schema);
    }

    // ---- T-049: response_format wiring at the body level ----

    fn test_ollama_provider() -> OllamaProvider {
        OllamaProvider::new(OllamaConfig::from_args(None, "llama3.1".into()))
    }

    #[test]
    fn ollama_body_sets_format_json_when_json_object() {
        // When the caller asks for JsonObject, the wire body
        // must set `format: "json"` (the Ollama-native shape).
        let p = test_ollama_provider();
        let req =
            InferenceRequest::new("hi", vec![]).with_response_format(ResponseFormat::JsonObject);
        let body = p.build_wire_body(&req);
        assert_eq!(body["format"], json!("json"));
    }

    #[test]
    fn ollama_body_sets_format_schema_when_json_schema() {
        // When the caller asks for JsonSchema, the wire body
        // must set `format` to the schema value verbatim.
        let p = test_ollama_provider();
        let schema = json!({
            "type": "object",
            "properties": {"answer": {"type": "string"}},
            "required": ["answer"],
        });
        let req = InferenceRequest::new("hi", vec![])
            .with_response_format(ResponseFormat::JsonSchema(schema.clone()));
        let body = p.build_wire_body(&req);
        assert_eq!(body["format"], schema);
    }

    #[test]
    fn ollama_body_omits_format_when_unset() {
        // When no response_format is supplied the `format`
        // field must be absent (Ollama defaults to plain text).
        let p = test_ollama_provider();
        let req = InferenceRequest::new("hi", vec![]);
        let body = p.build_wire_body(&req);
        assert!(
            body.get("format").is_none(),
            "format field must be absent when response_format is None: {body}"
        );
    }

    #[test]
    fn ollama_body_json_schema_preserves_nested_structure() {
        // Complex / nested schemas must round-trip verbatim
        // (no field dropping, no pretty-printing).
        let p = test_ollama_provider();
        let schema = json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {"id": {"type": "integer"}, "tags": {"type": "array", "items": {"type": "string"}}}
                    }
                }
            }
        });
        let req = InferenceRequest::new("hi", vec![])
            .with_response_format(ResponseFormat::JsonSchema(schema.clone()));
        let body = p.build_wire_body(&req);
        // The schema round-trips verbatim — we compare the
        // whole structure rather than drilling into nested
        // fields, which makes the test resilient to (legitimate)
        // changes in the schema shape.
        assert_eq!(body["format"], schema);
    }
}
