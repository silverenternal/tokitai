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
//!   "tools": [{"name": "...", "description": "...", "input_schema": {...}}]
//! }
//! ```
//!
//! The response uses Anthropic's `content` array of typed blocks;
//! the provider only cares about `text` and `tool_use` blocks.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokitai_core::ToolDefinition;

use super::{ChatMessage, CompletionResponse, Provider, ProviderToolCall, UsageReport};

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
        system: Option<&str>,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<CompletionResponse> {
        let wire_messages: Vec<Value> = messages.iter().map(anthropic_message).collect();
        let wire_tools: Vec<Value> = tools.iter().map(|t| t.to_anthropic_tool()).collect();

        let mut body = json!({
            "model": self.config.model,
            "messages": wire_messages,
            "tools": wire_tools,
            "max_tokens": self.config.max_tokens,
        });
        if let Some(sys) = system {
            body["system"] = json!(sys);
        }

        let mut req = self
            .client
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01");
        if !self.config.api_key.is_empty() {
            req = req.header("x-api-key", &self.config.api_key);
        }
        let resp = req.json(&body).send().await?;
        let parsed: AnthropicResponse = resp.error_for_status()?.json().await?;
        Ok(parsed.into_completion())
    }
}

// -- internal helpers ---------------------------------------------------

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
}
