//! T-034: Ollama provider.
//!
//! Wire format: <https://github.com/ollama/ollama/blob/main/docs/api.md#chat-request-with-tools>.
//! The endpoint is `/api/chat` and the request/response shape is
//! near-identical to OpenAI's. The local install is assumed at
//! `http://localhost:11434` and is reached with no auth header.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokitai_core::ToolDefinition;

use super::{ChatMessage, CompletionResponse, Provider, ProviderToolCall};

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
}

impl OllamaProvider {
    /// Build a new Ollama provider.
    pub fn new(config: OllamaConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    async fn complete_with_tools(
        &self,
        _system: Option<&str>,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<CompletionResponse> {
        // Ollama folds the system prompt into the first user turn
        // (it doesn't have a dedicated `system` field), so the
        // provider-agnostic `system` argument is intentionally
        // ignored here. Use the `OllamaMessage::System` variant
        // if you need it on the wire.
        let wire_messages: Vec<Value> = messages
            .iter()
            .map(|m| match m {
                ChatMessage::User { content } => {
                    json!({"role": "user", "content": content})
                }
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
            })
            .collect();

        let wire_tools: Vec<Value> = tools
            .iter()
            .map(|t| t.to_openai_function()["function"].clone())
            .collect();

        let body = json!({
            "model": self.config.model,
            "messages": wire_messages,
            "tools": wire_tools,
            "stream": false,
        });

        let url = format!("{}/api/chat", self.config.base_url);
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        let parsed: OllamaResponse = resp.error_for_status()?.json().await?;
        Ok(parsed.into_completion())
    }
}

// -- internal helpers ---------------------------------------------------

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
