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
//!   "tools":  [{"type":"function","function":{...}}],
//!   "tool_choice": "auto",
//!   "response_format": {"type":"json_object"},
//!   "temperature": 0.7,
//!   "max_tokens": 1024
//! }
//! ```
//!
//! T-043: every generation knob lives on `InferenceRequest`; the
//! provider threads them through to the wire body and drops
//! anything that is `None` (so the wire stays a faithful mirror of
//! the user request).
//!
//! The response is parsed back into a [`CompletionResponse`].

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    ChatMessage, CompletionResponse, InferenceRequest, Provider, ProviderToolCall, ResponseFormat,
    ToolChoice, UsageReport,
};

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
        req: &InferenceRequest,
    ) -> anyhow::Result<CompletionResponse> {
        // 1. Convert the provider-agnostic messages to the OpenAI shape.
        let mut wire_messages: Vec<Value> = Vec::with_capacity(req.messages.len() + 1);
        if let Some(sys) = req.system.as_deref() {
            wire_messages.push(json!({"role": "system", "content": sys}));
        }
        for m in &req.messages {
            wire_messages.push(openai_message(m));
        }

        // 2. Convert every ToolDefinition into the OpenAI `tools` array.
        let wire_tools: Vec<Value> = req.tools.iter().map(|t| t.to_openai_function()).collect();

        // 3. Build the body. Every T-043 knob is rendered only when
        //    the user actually set it so the wire stays a faithful
        //    mirror of the request.
        let mut body = json!({
            "model": self.config.model,
            "messages": wire_messages,
        });
        // Tools + tool_choice only matter when tools are present.
        // Sending `tool_choice: "none"` with an empty `tools` array
        // is a 400 on the OpenAI side.
        if !wire_tools.is_empty() {
            body["tools"] = json!(wire_tools);
            body["tool_choice"] = json!(openai_tool_choice(&req.tool_choice));
        }
        if let Some(max_tokens) = req.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temperature) = req.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(stop) = req.stop.as_ref() {
            body["stop"] = json!(stop);
        }
        if let Some(seed) = req.seed {
            body["seed"] = json!(seed);
        }
        if let Some(parallel) = req.parallel_tool_calls {
            body["parallel_tool_calls"] = json!(parallel);
        }
        if req.stream {
            body["stream"] = json!(true);
        }
        if let Some(rf) = req.response_format.as_ref() {
            body["response_format"] = json!(openai_response_format(rf));
        }

        // 4. POST. The bearer token is optional; a missing key
        //    is forwarded as an empty header (some local
        //    proxies accept that).
        let mut http_req = self
            .client
            .post(&self.api_url)
            .header("Content-Type", "application/json");
        if !self.config.api_key.is_empty() {
            http_req = http_req.bearer_auth(&self.config.api_key);
        }
        let resp = http_req.json(&body).send().await?;

        // 5. Parse the response. We use the typed `OpenAiResponse`
        //    struct for the bits we care about and fall back to
        //    `Value` for the tool-call argument blobs (which are
        //    already JSON strings on the wire).
        let parsed: OpenAiResponse = resp.error_for_status()?.json().await?;
        Ok(parsed.into_completion())
    }
}

// -- internal helpers ---------------------------------------------------

/// Render an `InferenceRequest::tool_choice` as the OpenAI wire
/// format. The OpenAI API expects one of:
/// - a string (`"auto"`, `"required"`, `"none"`), or
/// - `{"type":"function","function":{"name": "..."}}`.
fn openai_tool_choice(tc: &ToolChoice) -> Value {
    match tc {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::Required => json!("required"),
        ToolChoice::None => json!("none"),
        ToolChoice::Specific { name } => json!({
            "type": "function",
            "function": { "name": name }
        }),
    }
}

/// Render an `InferenceRequest::response_format` as the OpenAI
/// wire format. OpenAI accepts `{"type":"json_object"}` and
/// `{"type":"json_schema","json_schema":{...}}`; the schema body
/// is forwarded verbatim.
fn openai_response_format(rf: &ResponseFormat) -> Value {
    match rf {
        ResponseFormat::JsonObject => json!({"type": "json_object"}),
        ResponseFormat::JsonSchema(schema) => json!({
            "type": "json_schema",
            "json_schema": schema,
        }),
    }
}

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
    use crate::provider::ToolChoice;

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

    #[test]
    fn openai_tool_choice_wire_strings() {
        assert_eq!(openai_tool_choice(&ToolChoice::Auto), json!("auto"));
        assert_eq!(openai_tool_choice(&ToolChoice::Required), json!("required"));
        assert_eq!(openai_tool_choice(&ToolChoice::None), json!("none"));
    }

    #[test]
    fn openai_tool_choice_specific_renders_function_object() {
        let v = openai_tool_choice(&ToolChoice::Specific {
            name: "search_web".into(),
        });
        assert_eq!(v["type"], "function");
        assert_eq!(v["function"]["name"], "search_web");
    }

    #[test]
    fn openai_response_format_wire() {
        let v = openai_response_format(&ResponseFormat::JsonObject);
        assert_eq!(v, json!({"type": "json_object"}));

        let v = openai_response_format(&ResponseFormat::JsonSchema(json!({
            "name": "answer",
            "schema": {"type": "object"}
        })));
        assert_eq!(v["type"], "json_schema");
        assert_eq!(v["json_schema"]["schema"]["type"], "object");
    }
}
