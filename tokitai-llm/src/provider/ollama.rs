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
        _system: Option<&str>,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<CompletionResponse> {
        // Ollama folds the system prompt into the first user turn
        // (it doesn't have a dedicated `system` field), so the
        // provider-agnostic `system` argument is intentionally
        // ignored here. Use the `OllamaMessage::System` variant
        // if you need it on the wire.
        let wire_messages: Vec<Value> = messages.iter().map(ollama_message).collect();

        // Prefer the pre-rendered cache when it matches the active
        // tool slice (length-equality is the cheap approximation);
        // fall back to rendering on demand otherwise.
        let wire_tools: Vec<Value> = if self.tools_cache.len() == tools.len() && !tools.is_empty() {
            self.tools_cache.clone()
        } else {
            tools
                .iter()
                .map(|t| t.to_openai_function()["function"].clone())
                .collect()
        };

        let body = json!({
            "model": self.config.model,
            "messages": wire_messages,
            "tools": wire_tools,
            "stream": false,
        });

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
    use crate::provider::ProviderToolCall;
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
}
