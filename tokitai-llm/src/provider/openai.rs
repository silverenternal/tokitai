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
//!
//! T-044: streaming uses the same wire format with `stream: true`
//! on the request body. The server replies with a `text/event-stream`
//! of `data: {...}` records (one per delta). Each record carries
//! the partial `choices[0].delta` (text and/or `tool_calls`
//! fragments). The final record is the literal `data: [DONE]`. The
//! provider parses the stream into a sequence of
//! [`CompletionEvent::TextDelta`] and
//! [`CompletionEvent::ToolCallBegin`/`ToolCallArgsDelta`] events
//! and finishes with [`CompletionEvent::Done`].

use std::pin::Pin;

use async_stream::try_stream;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::sse::ParserState;
use super::{
    ChatMessage, CompletionEvent, CompletionEventStream, CompletionResponse, InferenceRequest,
    Provider, ProviderToolCall, ResponseFormat, ToolChoice, UsageReport,
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

    fn complete_streaming_native(
        &self,
        req: &InferenceRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<CompletionEventStream>> + Send + '_>>
    {
        // T-044: build the same wire body as `complete_with_tools`
        // but force `stream: true` regardless of the request knob
        // — the caller opted into streaming by reaching this
        // code path. The body fields are rendered the same way so
        // the model's reply is bit-identical to the non-streaming
        // path modulo the wire encoding.
        let mut wire_messages: Vec<Value> = Vec::with_capacity(req.messages.len() + 1);
        if let Some(sys) = req.system.as_deref() {
            wire_messages.push(json!({"role": "system", "content": sys}));
        }
        for m in &req.messages {
            wire_messages.push(openai_message(m));
        }
        let wire_tools: Vec<Value> = req.tools.iter().map(|t| t.to_openai_function()).collect();
        let mut body = json!({
            "model": self.config.model,
            "messages": wire_messages,
            "stream": true,
        });
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
        if let Some(rf) = req.response_format.as_ref() {
            body["response_format"] = json!(openai_response_format(rf));
        }

        let client = self.client.clone();
        let api_url = self.api_url.clone();
        let api_key = self.config.api_key.clone();
        let stream: CompletionEventStream = Box::pin(try_stream! {
            let mut http_req = client
                .post(&api_url)
                .header("Content-Type", "application/json");
            if !api_key.is_empty() {
                http_req = http_req.bearer_auth(&api_key);
            }
            let resp = http_req.json(&body).send().await?;
            let resp = resp.error_for_status()?;
            // `reqwest::Response::bytes_stream` returns
            // `Stream<Item = Result<Bytes, reqwest::Error>>`.
            // The test helper (`collect_openai_stream`) takes
            // `anyhow::Result<Bytes>` for parity with the rest
            // of the provider; map the error here so the
            // helper can stay provider-agnostic.
            let byte_stream = resp
                .bytes_stream()
                .map(|chunk| chunk.map_err(anyhow::Error::from));
            // Forward every event from the helper into the
            // async stream. The helper is a synchronous closure
            // so we can drive it inline.
            let mut collected: Vec<CompletionEvent> = Vec::new();
            openai_stream_events(byte_stream, &mut |ev| {
                collected.push(ev);
            })
            .await?;
            for ev in collected {
                yield ev;
            }
        });
        Box::pin(async move { Ok(stream) })
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

// -- T-044 streaming helpers --------------------------------------------

/// T-044: per-call accumulator for tool-call fragments keyed by
/// the OpenAI-assigned `index`. The OpenAI stream ships a single
/// fragment for each call in the first delta (id + name + empty
/// arguments) and a sequence of `arguments` deltas after.
#[derive(Debug, Default)]
struct OpenAiStreamingToolCall {
    /// Provider-assigned call id (some OpenAI-compatible
    /// servers omit it; we synthesise one in that case).
    id: Option<String>,
    /// Tool name (only set on the first fragment that carries
    /// the name).
    name: Option<String>,
    /// Concatenated argument JSON string (the dispatcher parses
    /// it as a single `serde_json::Value`).
    args: String,
}

/// T-044: typed shape of a single OpenAI streaming chunk. The
/// stream carries a `choices[0].delta` with optional `content`
/// and `tool_calls` arrays. The final chunk usually carries
/// `usage` (when the caller opted into the usage field).
#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: OpenAiStreamDelta,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiStreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiStreamToolCall>>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiStreamToolCall {
    /// OpenAI-assigned index; sometimes omitted on the first
    /// fragment (we default to 0 — the single-call case).
    #[serde(default)]
    index: Option<usize>,
    /// Call id (omitted on subsequent fragments).
    #[serde(default)]
    id: Option<String>,
    /// Function fragment.
    #[serde(default)]
    function: Option<OpenAiStreamFunction>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiStreamFunction {
    /// Tool name (omitted on subsequent fragments).
    #[serde(default)]
    name: Option<String>,
    /// Argument delta (JSON string fragment).
    #[serde(default)]
    arguments: Option<String>,
}

/// T-044: drive the SSE stream and yield `CompletionEvent`
/// values as deltas arrive. The function is the same code
/// path used by `complete_streaming_native`; tests reach it
/// via `collect_openai_stream`, which simply collects the
/// yielded events into a `Vec`.
async fn openai_stream_events<S>(
    byte_stream: S,
    yield_event: &mut (dyn FnMut(CompletionEvent) + Send),
) -> anyhow::Result<()>
where
    S: Stream<Item = anyhow::Result<Bytes>> + Unpin,
{
    let mut stream = byte_stream;
    let mut parser = ParserState::new();
    let mut content = String::new();
    let mut tool_calls: Vec<OpenAiStreamingToolCall> = Vec::new();
    let mut usage: Option<UsageReport> = None;
    let mut done_emitted = false;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        for ev in parser.feed(&chunk)? {
            if ev.done {
                done_emitted = true;
                continue;
            }
            let Some(data) = ev.data.as_ref() else {
                continue;
            };
            if data.is_empty() {
                continue;
            }
            let parsed: OpenAiStreamChunk = serde_json::from_str(data)?;
            for choice in parsed.choices {
                if let Some(text) = choice.delta.content {
                    if !text.is_empty() {
                        content.push_str(&text);
                        yield_event(CompletionEvent::TextDelta(text));
                    }
                }
                if let Some(fragments) = choice.delta.tool_calls {
                    for frag in fragments {
                        let idx = frag.index.unwrap_or(0);
                        while tool_calls.len() <= idx {
                            tool_calls.push(OpenAiStreamingToolCall::default());
                        }
                        let entry = &mut tool_calls[idx];
                        if let Some(id) = frag.id.clone() {
                            entry.id = Some(id);
                        }
                        if let Some(name) = frag.function.as_ref().and_then(|f| f.name.as_ref()) {
                            if entry.name.is_none() {
                                entry.name = Some(name.clone());
                                yield_event(CompletionEvent::ToolCallBegin {
                                    index: idx,
                                    name: name.clone(),
                                    id: entry.id.clone(),
                                });
                            }
                        }
                        if let Some(args) =
                            frag.function.as_ref().and_then(|f| f.arguments.as_ref())
                        {
                            if !args.is_empty() {
                                entry.args.push_str(args);
                                yield_event(CompletionEvent::ToolCallArgsDelta {
                                    index: idx,
                                    args: args.clone(),
                                });
                            }
                        }
                    }
                }
            }
            if let Some(u) = parsed.usage {
                usage = Some(UsageReport {
                    prompt_tokens: u.prompt_tokens,
                    completion_tokens: u.completion_tokens,
                    total_tokens: u.total_tokens,
                });
            }
        }
        if done_emitted {
            break;
        }
    }
    for ev in parser.flush()? {
        if ev.done {
            done_emitted = true;
        }
    }
    let final_tool_calls: Vec<ProviderToolCall> = tool_calls
        .into_iter()
        .enumerate()
        .filter_map(|(idx, tc)| {
            let name = tc.name?;
            let id = tc.id.unwrap_or_else(|| format!("openai_call_{idx}"));
            let arguments = match serde_json::from_str(&tc.args) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "openai stream: failed to parse tool call arguments for {name}: {e}"
                    );
                    Value::Null
                }
            };
            Some(ProviderToolCall {
                id,
                name,
                arguments,
            })
        })
        .collect();
    let _ = done_emitted;
    yield_event(CompletionEvent::Done(CompletionResponse {
        content,
        tool_calls: final_tool_calls,
        usage,
    }));
    Ok(())
}

/// T-044: test helper. Collects every `CompletionEvent` the
/// SSE stream produces into a `Vec`. Production code reaches
/// the same `openai_stream_events` closure through
/// `complete_streaming_native`.
#[cfg(test)]
pub(crate) async fn collect_openai_stream<S>(byte_stream: S) -> anyhow::Result<Vec<CompletionEvent>>
where
    S: Stream<Item = anyhow::Result<Bytes>> + Unpin,
{
    let mut events: Vec<CompletionEvent> = Vec::new();
    openai_stream_events(byte_stream, &mut |ev| events.push(ev)).await?;
    Ok(events)
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

    // -- T-044 streaming tests ---------------------------------------

    use bytes::Bytes;
    use futures::stream;

    /// T-044: build a `Stream<Item = Result<Bytes>>` from a
    /// list of pre-rendered SSE chunks. The helper is the
    /// mirror of `reqwest::Response::bytes_stream` so the
    /// provider's streaming code can be exercised without a
    /// live HTTP server.
    fn sse_chunks(chunks: &[&'static str]) -> impl Stream<Item = anyhow::Result<Bytes>> {
        let items: Vec<anyhow::Result<Bytes>> =
            chunks.iter().map(|c| Ok(Bytes::from(*c))).collect();
        stream::iter(items)
    }

    #[tokio::test]
    async fn openai_stream_text_only() {
        // A minimal OpenAI stream: one role-only chunk, two
        // text-delta chunks, then `[DONE]`.
        let chunks = &[
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\", world!\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{}}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\n",
            "data: [DONE]\n\n",
        ];
        let events = collect_openai_stream(sse_chunks(chunks)).await.unwrap();
        // Two text deltas + final Done. The role-only and
        // empty-delta chunks are not surfaced.
        let text_deltas: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                CompletionEvent::TextDelta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text_deltas, vec!["Hello", ", world!"]);
        let final_response = events
            .iter()
            .find_map(|e| match e {
                CompletionEvent::Done(r) => Some(r),
                _ => None,
            })
            .expect("Done event");
        assert_eq!(final_response.content, "Hello, world!");
        assert!(final_response.tool_calls.is_empty());
        let usage = final_response.usage.as_ref().expect("usage");
        assert_eq!(usage.prompt_tokens, 3);
        assert_eq!(usage.completion_tokens, 2);
        assert_eq!(usage.total_tokens, 5);
    }

    #[tokio::test]
    async fn openai_stream_tool_call() {
        // A single tool call: first fragment carries id+name
        // + empty arguments, subsequent fragments carry
        // argument deltas, then `[DONE]`.
        let chunks = &[
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"echo\",\"arguments\":\"\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"x\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1}\"}}]}}]}\n\n",
            "data: [DONE]\n\n",
        ];
        let events = collect_openai_stream(sse_chunks(chunks)).await.unwrap();
        // One ToolCallBegin + two ToolCallArgsDelta + Done.
        let begins: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                CompletionEvent::ToolCallBegin { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(begins, vec!["echo"]);
        let arg_deltas: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                CompletionEvent::ToolCallArgsDelta { args, .. } => Some(args.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(arg_deltas, vec!["{\"x\":", "1}"]);
        let final_response = events
            .iter()
            .find_map(|e| match e {
                CompletionEvent::Done(r) => Some(r),
                _ => None,
            })
            .expect("Done event");
        assert_eq!(final_response.tool_calls.len(), 1);
        assert_eq!(final_response.tool_calls[0].id, "call_1");
        assert_eq!(final_response.tool_calls[0].name, "echo");
        assert_eq!(final_response.tool_calls[0].arguments, json!({"x": 1}));
    }

    #[tokio::test]
    async fn openai_stream_split_chunks() {
        // Verify the parser handles records that straddle
        // chunk boundaries. We split the wire so the first
        // `data:` line is split across two chunks.
        let chunks = &[
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hel",
            "lo\"}}]}\n\ndata: [DONE]\n\n",
        ];
        let events = collect_openai_stream(sse_chunks(chunks)).await.unwrap();
        let text_deltas: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                CompletionEvent::TextDelta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text_deltas, vec!["Hello"]);
    }
}
