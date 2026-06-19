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

/// Configuration for the Anthropic provider.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    /// Base URL without trailing slash.
    pub base_url: String,
    /// Model name (e.g. `claude-3-5-sonnet-latest`).
    pub model: String,
    /// API key (sent in the `x-api-key` header, NOT Authorization).
    pub api_key: String,
}

impl AnthropicConfig {
    /// Build a config from CLI args.
    pub fn from_args(base_url: Option<String>, model: String, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            model,
            api_key: api_key.unwrap_or_default(),
        }
    }
}

/// Anthropic Messages provider.
pub struct AnthropicProvider {
    config: AnthropicConfig,
    client: Client,
}

impl AnthropicProvider {
    /// Build a new Anthropic provider.
    pub fn new(config: AnthropicConfig) -> Self {
        Self {
            config,
            client: Client::new(),
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
            "max_tokens": 1024u32,
        });
        if let Some(sys) = system {
            body["system"] = json!(sys);
        }

        let url = format!("{}/v1/messages", self.config.base_url);
        let mut req = self
            .client
            .post(&url)
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
}
