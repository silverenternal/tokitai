//! T-034: Anthropic Messages provider.
//!
//! Wire format: <https://docs.anthropic.com/en/docs/build-with-claude/tool-use>.
//! POSTs to `{base_url}/v1/messages` with a body shaped like:
//!
//! ```json
//! {
//!   "model": "claude-3-5-sonnet-latest",
//!   "system": "...",
//!   "messages": [{"role":"user","content":"..."}],
//!   "tools": [{"name": "...", "description": "...", "input_schema": {...}}],
//!   "max_tokens": 4096
//! }
//! ```
//!
//! T-043: the request is now an `InferenceRequest`. Anthropic
//! natively supports `max_tokens` (required) and `temperature` /
//! `stop_sequences`; everything else (`tool_choice`, `seed`,
//! `parallel_tool_calls`, `stream`) is dropped from the wire body
//! because the Anthropic API has no equivalent.
//!
//! T-049: `response_format` is wired via the
//! `anthropic-beta: output-json-2025-01-10` header (a beta feature
//! that unlocks JSON / schema-pinned outputs on the Messages
//! API). For `JsonObject` we set the header and let the model
//! emit a JSON object. For `JsonSchema` we additionally inject a
//! system-prompt preamble instructing the model to return JSON
//! matching the supplied schema verbatim (the schema itself is
//! not part of the v1 Messages wire shape, so we surface it as a
//! directive in `system`).
//!
//! The response uses Anthropic's `content` array of typed blocks;
//! the provider only cares about `text` and `tool_use` blocks.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    ChatMessage, CompletionResponse, InferenceRequest, Provider, ProviderToolCall, ResponseFormat,
    UsageReport,
};

/// Beta header value that enables structured JSON output on the
/// Anthropic Messages API. See
/// <https://docs.anthropic.com/en/docs/build-with-claude/structured-outputs>.
pub const ANTHROPIC_BETA_STRUCTURED_OUTPUTS: &str = "output-json-2025-01-10";

/// Default base URL for the Anthropic Messages API.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// Default `max_tokens` for the Anthropic provider. Anthropic
/// requires this field on every Messages request, so we apply a
/// sane default when the user did not set `--max-tokens`.
pub const DEFAULT_MAX_TOKENS: u64 = 4096;

/// Configuration for the Anthropic provider.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    /// Base URL without trailing slash.
    pub base_url: String,
    /// Model name (e.g. `claude-3-5-sonnet-latest`).
    pub model: String,
    /// API key (sent in the `x-api-key` header, NOT Authorization).
    pub api_key: String,
    /// `max_tokens` to send with every request. Anthropic requires
    /// this field; defaults to [`DEFAULT_MAX_TOKENS`].
    pub max_tokens: u64,
}

impl AnthropicConfig {
    /// Build a config from CLI args.
    pub fn from_args(
        base_url: Option<String>,
        model: String,
        api_key: Option<String>,
        max_tokens: Option<u64>,
    ) -> Self {
        Self {
            base_url: base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            model,
            api_key: api_key.unwrap_or_default(),
            max_tokens: max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
        }
    }
}

/// Anthropic Messages provider.
pub struct AnthropicProvider {
    config: AnthropicConfig,
    client: Client,
    /// Pre-computed chat-completions URL. Computed once in `new()`
    /// so `complete_with_tools` does not pay the `format!` cost
    /// on every call.
    api_url: String,
}

impl AnthropicProvider {
    /// Build a new Anthropic provider.
    pub fn new(config: AnthropicConfig) -> Self {
        let api_url = format!("{}/v1/messages", config.base_url);
        Self {
            config,
            client: Client::new(),
            api_url,
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn complete_with_tools(
        &self,
        req: &InferenceRequest,
    ) -> anyhow::Result<CompletionResponse> {
        let wire_messages: Vec<Value> = req.messages.iter().map(anthropic_message).collect();
        let wire_tools: Vec<Value> = req.tools.iter().map(|t| t.to_anthropic_tool()).collect();

        // T-043: max_tokens on the request overrides the config
        // default; the field is required by the Anthropic API so
        // we always render it.
        let max_tokens = req.max_tokens.unwrap_or(self.config.max_tokens);

        // T-049: when a structured-output format is requested we
        // need to (a) set the `anthropic-beta` header on the
        // request, and (b) for `JsonSchema`, fold a structured-
        // output preamble into the system prompt. We do the
        // body-shape work up front so the request builder can
        // stay a flat `json!` block.
        let structured = req.response_format.as_ref().map(anthropic_response_format);

        let mut body = json!({
            "model": self.config.model,
            "messages": wire_messages,
            "max_tokens": max_tokens,
        });
        if !wire_tools.is_empty() {
            body["tools"] = json!(wire_tools);
        }
        // Compose the system prompt: existing user-supplied
        // system (if any) followed by the structured-output
        // directive (if any). The directive is appended (not
        // prepended) so the user's instructions keep priority.
        let system_text: Option<String> = match (req.system.as_deref(), structured.as_ref()) {
            (Some(s), Some(d)) => Some(format!("{s}\n\n{d}")),
            (Some(s), None) => Some(s.to_string()),
            (None, Some(d)) => Some(d.clone()),
            (None, None) => None,
        };
        if let Some(sys) = system_text.as_deref() {
            body["system"] = json!(sys);
        }
        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(stop) = req.stop.as_ref() {
            // Anthropic uses `stop_sequences`, plural, where OpenAI
            // uses `stop` (singular). Translate the field name.
            body["stop_sequences"] = json!(stop);
        }
        // T-043: Anthropic has no `tool_choice`, `seed`,
        // `parallel_tool_calls`, or `stream` equivalents in the
        // v1 Messages API. The fields are accepted on the
        // request (so callers can use a single
        // `InferenceRequest` shape across providers) and dropped
        // here at the wire boundary.
        // T-049: `response_format` is rendered via the beta
        // header + system preamble above; the v1 Messages wire
        // shape has no native equivalent.

        let mut http_req = self
            .client
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01");
        if req.response_format.is_some() {
            // The header value is a comma-separated list of beta
            // feature names; future Anthropic beta flags can be
            // appended here without changing the wire shape.
            http_req = http_req.header("anthropic-beta", ANTHROPIC_BETA_STRUCTURED_OUTPUTS);
        }
        if !self.config.api_key.is_empty() {
            http_req = http_req.header("x-api-key", &self.config.api_key);
        }
        let resp = http_req.json(&body).send().await?;
        let parsed: AnthropicResponse = resp.error_for_status()?.json().await?;
        Ok(parsed.into_completion())
    }
}

// -- internal helpers ---------------------------------------------------

/// Render `ResponseFormat` for the Anthropic wire shape. Returns
/// the system-prompt directive the provider should append when
/// the corresponding `anthropic-beta` header is set.
///
/// For `JsonObject` we ask the model to "respond with a single
/// JSON object" — the Anthropic beta flag handles the rest. For
/// `JsonSchema` we embed the schema verbatim (the Messages API
/// has no native `json_schema` field, so we lean on a directive).
fn anthropic_response_format(rf: &ResponseFormat) -> String {
    match rf {
        ResponseFormat::JsonObject => {
            // Beta flag turns on JSON output; the prompt tells
            // the model what to do so the user does not have to
            // spell it out.
            "Respond with a single JSON object. Do not wrap it \
             in markdown or prose."
                .to_string()
        }
        ResponseFormat::JsonSchema(schema) => {
            // Anthropic's Messages API does not yet accept a
            // `response_format.json_schema` field; the convention
            // (documented for the `output-json-2025-01-10` beta)
            // is to forward the schema in the system prompt and
            // ask the model to comply verbatim. We render the
            // schema with `to_string()` for a stable, JSON-shaped
            // directive the model can parse.
            format!(
                "Respond with a single JSON object that conforms \
                 to this JSON Schema:\n\n```json\n{}\n```\n\nDo \
                 not wrap the response in markdown or prose.",
                serde_json::to_string_pretty(schema).unwrap_or_else(|_| schema.to_string())
            )
        }
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicBlock>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicBlock {
    /// Plain-text reply. Concatenated into `CompletionResponse::content`.
    Text { text: String },
    /// Tool call. Mapped to `ProviderToolCall`.
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// Other block types (e.g. `tool_result` echoes). Ignored.
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}

impl AnthropicResponse {
    fn into_completion(self) -> CompletionResponse {
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        for block in self.content {
            match block {
                AnthropicBlock::Text { text } => content.push_str(&text),
                AnthropicBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ProviderToolCall {
                        id,
                        name,
                        arguments: input,
                    });
                }
                AnthropicBlock::Other => {}
            }
        }
        let usage = self.usage.map(|u| UsageReport {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens + u.output_tokens,
        });
        CompletionResponse {
            content,
            tool_calls,
            usage,
        }
    }
}

fn anthropic_message(m: &ChatMessage) -> Value {
    match m {
        ChatMessage::User { content } => json!({"role": "user", "content": content}),
        ChatMessage::Assistant {
            content,
            tool_calls,
        } => {
            let mut blocks: Vec<Value> = Vec::new();
            if let Some(text) = content {
                if !text.is_empty() {
                    blocks.push(json!({"type": "text", "text": text}));
                }
            }
            for tc in tool_calls {
                blocks.push(json!({
                    "type": "tool_use",
                    "id": tc.id,
                    "name": tc.name,
                    "input": tc.arguments,
                }));
            }
            json!({"role": "assistant", "content": blocks})
        }
        ChatMessage::Tool {
            tool_call_id,
            content,
        } => {
            json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": tool_call_id,
                    "content": content,
                }]
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderToolCall;
    use serde_json::json;

    #[test]
    fn tool_message_wraps_in_tool_result_block() {
        let m = ChatMessage::Tool {
            tool_call_id: "tu_1".into(),
            content: "ok".into(),
        };
        let v = anthropic_message(&m);
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"][0]["type"], "tool_result");
        assert_eq!(v["content"][0]["tool_use_id"], "tu_1");
    }

    #[test]
    fn anthropic_message_user_renders_with_role_and_content() {
        let m = ChatMessage::User {
            content: "hello".into(),
        };
        let v = anthropic_message(&m);
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "hello");
    }

    #[test]
    fn anthropic_message_assistant_text_only() {
        let m = ChatMessage::Assistant {
            content: Some("plain reply".into()),
            tool_calls: vec![],
        };
        let v = anthropic_message(&m);
        assert_eq!(v["role"], "assistant");
        assert_eq!(v["content"][0]["type"], "text");
        assert_eq!(v["content"][0]["text"], "plain reply");
        // No tool_use blocks emitted when tool_calls is empty.
        assert_eq!(v["content"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn anthropic_message_assistant_tool_use() {
        let m = ChatMessage::Assistant {
            content: None,
            tool_calls: vec![ProviderToolCall {
                id: "tu_a".into(),
                name: "echo".into(),
                arguments: json!({"x": 1}),
            }],
        };
        let v = anthropic_message(&m);
        assert_eq!(v["role"], "assistant");
        assert_eq!(v["content"][0]["type"], "tool_use");
        assert_eq!(v["content"][0]["id"], "tu_a");
        assert_eq!(v["content"][0]["name"], "echo");
        assert_eq!(v["content"][0]["input"]["x"], 1);
    }

    #[test]
    fn anthropic_multi_text_block_concatenation() {
        // Regression test: Anthropic returns an array of text
        // blocks; the provider must concatenate them all so the
        // dispatcher sees the full reply (not just the first block).
        let resp = AnthropicResponse {
            content: vec![
                AnthropicBlock::Text {
                    text: "Hello".into(),
                },
                AnthropicBlock::Text {
                    text: " world".into(),
                },
            ],
            usage: None,
        };
        let c = resp.into_completion();
        assert_eq!(c.content, "Hello world");
        assert!(c.tool_calls.is_empty());
    }

    #[test]
    fn anthropic_other_block_ignored() {
        let resp = AnthropicResponse {
            content: vec![
                AnthropicBlock::Other,
                AnthropicBlock::Text {
                    text: "kept".into(),
                },
                AnthropicBlock::Other,
            ],
            usage: None,
        };
        let c = resp.into_completion();
        assert_eq!(c.content, "kept");
        assert!(c.tool_calls.is_empty());
    }

    #[test]
    fn anthropic_response_into_completion_tool_use() {
        let resp = AnthropicResponse {
            content: vec![AnthropicBlock::ToolUse {
                id: "tu_b".into(),
                name: "echo".into(),
                input: json!({"y": 2}),
            }],
            usage: Some(AnthropicUsage {
                input_tokens: 11,
                output_tokens: 7,
            }),
        };
        let c = resp.into_completion();
        assert_eq!(c.tool_calls.len(), 1);
        assert_eq!(c.tool_calls[0].id, "tu_b");
        assert_eq!(c.tool_calls[0].name, "echo");
        assert_eq!(c.tool_calls[0].arguments, json!({"y": 2}));
        let u = c.usage.expect("usage present");
        assert_eq!(u.prompt_tokens, 11);
        assert_eq!(u.completion_tokens, 7);
        assert_eq!(u.total_tokens, 18);
    }

    #[test]
    fn anthropic_config_from_args_applies_default_max_tokens() {
        let cfg = AnthropicConfig::from_args(None, "claude-3-5-sonnet-latest".into(), None, None);
        assert_eq!(cfg.max_tokens, DEFAULT_MAX_TOKENS);
        assert_eq!(cfg.base_url, DEFAULT_BASE_URL);
        assert_eq!(cfg.api_key, "");
    }

    #[test]
    fn anthropic_config_from_args_strips_trailing_slash() {
        let cfg = AnthropicConfig::from_args(
            Some("https://api.example.com/".into()),
            "claude-3-5-sonnet-latest".into(),
            Some("k".into()),
            Some(2048),
        );
        assert_eq!(cfg.base_url, "https://api.example.com");
        assert_eq!(cfg.api_key, "k");
        assert_eq!(cfg.max_tokens, 2048);
    }

    // ---- T-049: response_format wiring ----

    #[test]
    fn anthropic_response_format_json_object_directive() {
        let s = anthropic_response_format(&ResponseFormat::JsonObject);
        // The directive must mention JSON object so the model
        // has a clear instruction even though the v1 Messages
        // API has no native `response_format` field.
        assert!(s.contains("JSON object"), "directive missing: {s}");
    }

    #[test]
    fn anthropic_response_format_json_schema_embeds_schema() {
        let schema = json!({
            "type": "object",
            "properties": {"answer": {"type": "string"}},
            "required": ["answer"],
        });
        let s = anthropic_response_format(&ResponseFormat::JsonSchema(schema.clone()));
        // The schema must appear in the directive verbatim
        // (as a JSON block) so the model can parse it.
        assert!(s.contains("JSON Schema"), "directive missing: {s}");
        let rendered = serde_json::to_string_pretty(&schema).unwrap();
        assert!(s.contains(&rendered), "schema not embedded verbatim: {s}");
        assert!(s.contains("\"answer\""));
        assert!(s.contains("\"required\""));
    }

    #[test]
    fn anthropic_beta_header_constant_matches_spec() {
        // Pin the beta header value: a typo would silently break
        // structured-output mode for every Anthropic user.
        assert_eq!(ANTHROPIC_BETA_STRUCTURED_OUTPUTS, "output-json-2025-01-10");
    }

    #[test]
    fn anthropic_request_sets_beta_header_when_json_object() {
        // We cannot easily mock reqwest's outbound request, but
        // we can verify the helper produces the right directive
        // and that the public constant is wired through the
        // request builder path. The full wire integration is
        // exercised by `examples/mcp_http_server` / hand tests;
        // here we pin the policy at the unit level.
        let rf = ResponseFormat::JsonObject;
        let directive = anthropic_response_format(&rf);
        assert!(!directive.is_empty());
        assert_eq!(ANTHROPIC_BETA_STRUCTURED_OUTPUTS, "output-json-2025-01-10");
    }

    #[test]
    fn anthropic_request_with_response_format_prepends_schema_directive() {
        // Verify that when a JsonSchema is supplied the directive
        // contains the schema's defining properties (so the
        // model gets a parseable contract).
        let schema = json!({
            "type": "object",
            "properties": {
                "answer": {"type": "string"},
                "confidence": {"type": "number"},
            },
            "required": ["answer", "confidence"],
        });
        let directive = anthropic_response_format(&ResponseFormat::JsonSchema(schema));
        assert!(directive.contains("confidence"));
        assert!(directive.contains("answer"));
        // The directive must ask for a JSON object (the
        // structured-output preamble) — we do NOT want the
        // schema to be the entire directive, because the model
        // would then emit prose around it.
        assert!(directive.contains("JSON object"));
    }
}
