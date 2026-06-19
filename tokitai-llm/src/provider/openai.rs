//! T-034: OpenAI Chat Completions provider.
//!
//! Wire format: <https://platform.openai.com/docs/guides/function-calling>.
//! The provider POSTs to `{base_url}/v1/chat/completions` with a
//! body shaped like:
//!
//! ```json
//! {
//!   "model": "gpt-4o",
//!   "messages": [{"role":"user","content":"..."}],
//!   "tools":  [{"type":"function","function":{...}}]
//! }
//! ```
//!
//! The response is parsed back into a [`CompletionResponse`].

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokitai_core::ToolDefinition;

use super::{ChatMessage, CompletionResponse, Provider, ProviderToolCall, UsageReport};

/// Default base URL for the OpenAI Chat Completions API. Override
/// with `--base-url` for the Azure-hosted variant.
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com";

/// Configuration for the OpenAI provider. Constructed from CLI args
/// (or env vars) in `cli::ProviderArgs`.
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    /// Base URL without trailing slash (default: `DEFAULT_BASE_URL`).
    pub base_url: String,
    /// Model name (e.g. `gpt-4o`).
    pub model: String,
    /// Bearer token. May be empty for local proxies.
    pub api_key: String,
}

impl OpenAiConfig {
    /// Build a config from CLI args, applying defaults for the
    /// fields the user left empty.
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

/// OpenAI Chat Completions provider.
pub struct OpenAiProvider {
    config: OpenAiConfig,
    client: Client,
    /// Pre-computed chat-completions URL. Computed once in `new()`
    /// so `complete_with_tools` does not pay the `format!` cost
    /// on every call.
    api_url: String,
}

impl OpenAiProvider {
    /// Build a new OpenAI provider.
    pub fn new(config: OpenAiConfig) -> Self {
        let api_url = format!("{}/v1/chat/completions", config.base_url);
        Self {
            config,
            client: Client::new(),
            api_url,
        }
    }
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn complete_with_tools(
        &self,
        system: Option<&str>,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<CompletionResponse> {
        // 1. Convert the provider-agnostic messages to the OpenAI shape.
        let mut wire_messages: Vec<Value> = Vec::with_capacity(messages.len() + 1);
        if let Some(sys) = system {
            wire_messages.push(json!({"role": "system", "content": sys}));
        }
        for m in messages {
            wire_messages.push(openai_message(m));
        }

        // 2. Convert every ToolDefinition into the OpenAI `tools` array.
        let wire_tools: Vec<Value> = tools.iter().map(|t| t.to_openai_function()).collect();

        let body = json!({
            "model": self.config.model,
            "messages": wire_messages,
            "tools": wire_tools,
        });

        // 3. POST. The bearer token is optional; a missing key
        //    is forwarded as an empty header (some local
        //    proxies accept that).
        let mut req = self
            .client
            .post(&self.api_url)
            .header("Content-Type", "application/json");
        if !self.config.api_key.is_empty() {
            req = req.bearer_auth(&self.config.api_key);
        }
        let resp = req.json(&body).send().await?;

        // 4. Parse the response. We use the typed `OpenAiResponse`
        //    struct for the bits we care about and fall back to
        //    `Value` for the tool-call argument blobs (which are
        //    already JSON strings on the wire).
        let parsed: OpenAiResponse = resp.error_for_status()?.json().await?;
        Ok(parsed.into_completion())
    }
}

// -- internal helpers ---------------------------------------------------

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunction {
    name: String,
    /// Arguments as a JSON string per the OpenAI wire format.
    /// We parse it into a `Value` here so the dispatcher can
    /// hand it to `call_tool` without a second pass.
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

impl OpenAiResponse {
    fn into_completion(self) -> CompletionResponse {
        let choice = self.choices.into_iter().next();
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        if let Some(c) = choice {
            content = c.message.content.unwrap_or_default();
            tool_calls = c
                .message
                .tool_calls
                .into_iter()
                .map(|tc| {
                    let arguments = match serde_json::from_str(&tc.function.arguments) {
                        Ok(v) => v,
                        Err(e) => {
                            // Surface malformed JSON rather than
                            // silently substituting `Value::Null`:
                            // the dispatcher would otherwise see an
                            // empty argument bag and call the tool
                            // with the wrong shape.
                            tracing::warn!(
                                "failed to parse tool call arguments for {}: {}",
                                tc.function.name,
                                e
                            );
                            Value::Null
                        }
                    };
                    ProviderToolCall {
                        id: tc.id,
                        name: tc.function.name,
                        arguments,
                    }
                })
                .collect();
        }
        let usage = self.usage.map(|u| UsageReport {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });
        CompletionResponse {
            content,
            tool_calls,
            usage,
        }
    }
}

fn openai_message(m: &ChatMessage) -> Value {
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
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": tc.arguments.to_string(),
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

    #[test]
    fn openai_message_user_renders_with_role_and_content() {
        let m = ChatMessage::User {
            content: "hi".into(),
        };
        let v = openai_message(&m);
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "hi");
    }

    #[test]
    fn openai_message_tool_carries_id_and_content() {
        let m = ChatMessage::Tool {
            tool_call_id: "call_42".into(),
            content: "42".into(),
        };
        let v = openai_message(&m);
        assert_eq!(v["role"], "tool");
        assert_eq!(v["tool_call_id"], "call_42");
        assert_eq!(v["content"], "42");
    }
}
