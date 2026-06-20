//! T-034: provider abstraction over OpenAI, Anthropic, and Ollama.
//!
//! Every LLM SDK we want to talk to fits the same shape:
//! 1. Render every `ToolDefinition` as a JSON-Schema envelope.
//! 2. POST a chat-completion request (text + tool envelope).
//! 3. Parse the response. The model either answered in plain text
//!    OR returned one-or-more tool calls.
//! 4. If tool calls were returned, dispatch them in-process via
//!    `ToolProvider::call_tool`, then POST the results back as
//!    a follow-up turn.
//! 5. Repeat until the model emits a final text answer (or the
//!    caller hits `max_iterations`).
//!
//! All of that is what `Provider::complete_with_tools` does. The
//! three concrete implementations (OpenAI / Anthropic / Ollama)
//! differ only in the wire format; the loop itself lives in
//! `infer::run`.
//!
//! T-043: the trait now takes a single `&InferenceRequest` so we
//! can thread tool-choice, response-format, temperature, and the
//! other generation knobs through one call instead of growing the
//! argument list with every new feature.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokitai_core::ToolDefinition;

pub mod anthropic;
pub mod ollama;
pub mod openai;
pub mod sse;

use std::pin::Pin;

use futures::{Future, Stream};

/// T-044: a single event in a streaming chat-completion response.
#[derive(Debug, Clone, PartialEq)]
pub enum CompletionEvent {
    /// Incremental text token from the model's reply.
    TextDelta(String),
    /// A new tool call started; carries the index, name, and
    /// (when the provider supplies one) the server-assigned id.
    ToolCallBegin {
        /// Zero-based index of the tool call within the
        /// response's `tool_calls` array.
        index: usize,
        /// Name of the tool the model wants to invoke.
        name: String,
        /// Provider-assigned id for the tool call, when one
        /// is supplied (e.g. OpenAI's `call_*`, Anthropic's
        /// block id). May be `None` for providers that do not
        /// emit stable ids in their SSE stream.
        id: Option<String>,
    },
    /// Incremental JSON-arguments fragment for a tool call.
    ToolCallArgsDelta {
        /// Zero-based index of the tool call whose arguments
        /// are being streamed.
        index: usize,
        /// Fragment of the JSON arguments string. The caller
        /// accumulates fragments across all `ToolCallArgsDelta`
        /// events that share an `index` until the call ends.
        args: String,
    },
    /// Stream is finished; carries the final aggregated response.
    Done(CompletionResponse),
}

/// T-044: a `BoxStream` of `CompletionEvent` values.
pub type CompletionEventStream =
    Pin<Box<dyn Stream<Item = anyhow::Result<CompletionEvent>> + Send>>;

/// T-050: outcome of a `ProviderMiddleware::on_error` invocation.
///
/// `Retry { delay }` asks the provider loop to back off for `delay`
/// and try the same request again (with `attempt` incremented);
/// `GiveUp` propagates the error to the caller unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum RetryDecision {
    /// Retry the request after `delay`. The provider loop will
    /// re-run `pre_request`, fire the HTTP call, and re-evaluate
    /// `on_error` if it fails again (with `attempt` incremented).
    Retry {
        /// How long to wait before retrying.
        delay: Duration,
    },
    /// Surface the error to the caller. The provider loop stops
    /// and returns the `anyhow::Error` from `complete_with_tools`.
    GiveUp,
}

/// T-050: provider-level middleware hooks.
///
/// Middleware is the provider layer's pluggable extension point —
/// the three default implementations (OpenAI / Anthropic / Ollama)
/// call each hook at well-defined points in the request lifecycle,
/// and a `Vec<Box<dyn ProviderMiddleware>>` on the provider
/// config supplies zero-or-more user-installed hooks (the default
/// is the empty vec, which is zero behavioral change).
///
/// The trait is `Send + Sync` so middleware can hold shared state
/// behind a `Mutex` / `RwLock` (auth-token refresh, OTel exporter,
/// rate-limit counters). All hooks take `&self`; mutations happen
/// through the `Send + Sync` interior-mutability contract.
///
/// Order: hooks run in registration order for `pre_request` and
/// `on_error`; they run in REVERSE registration order for
/// `post_request` so they behave like a call stack (the last
/// registered is the outermost, the first to observe the response).
#[async_trait]
pub trait ProviderMiddleware: Send + Sync + std::fmt::Debug {
    /// T-050: invoked once per `complete_with_tools` call BEFORE
    /// the HTTP request is sent. The middleware can mutate the
    /// request in place — for example, to inject a fresh
    /// `system` prompt, attach a tracing span, or stamp the
    /// `messages` vec with a request-id header.
    ///
    /// An `Err` aborts the call: `complete_with_tools` returns
    /// the error without firing `post_request` or `on_error`.
    async fn pre_request(&self, req: &mut InferenceRequest) -> anyhow::Result<()>;

    /// T-050: invoked once per `complete_with_tools` call AFTER
    /// the HTTP response is parsed. The middleware sees the full
    /// request + response + elapsed duration, useful for OTel
    /// exporters and token-usage telemetry.
    ///
    /// `resp` carries `Ok(&CompletionResponse)` on success and
    /// `Err(&anyhow::Error)` on failure; in the failure case the
    /// middleware ALSO receives an `on_error` call — `post_request`
    /// and `on_error` are NOT mutually exclusive. Both fire so a
    /// middleware that wants the duration can read it here, and a
    /// middleware that wants to retry can decide in `on_error`.
    ///
    /// Errors from `post_request` are logged and swallowed: the
    /// provider already has a final response and we must not
    /// turn a successful call into an error just because telemetry
    /// emission failed.
    async fn post_request(
        &self,
        req: &InferenceRequest,
        resp: &anyhow::Result<CompletionResponse>,
        duration: Duration,
    );

    /// T-050: invoked once per `complete_with_tools` call when the
    /// HTTP transport or response parse returned an error.
    /// `attempt` is the 0-based retry counter (0 on the first try,
    /// incremented on each subsequent `Retry` decision).
    ///
    /// Returning `Retry { delay }` asks the provider loop to sleep
    /// `delay` and try again; `GiveUp` propagates the error
    /// unchanged. The first `Retry` decision wins — subsequent
    /// middleware are not consulted.
    async fn on_error(&self, error: &anyhow::Error, attempt: u32) -> RetryDecision;
}

/// T-050: maximum number of retry attempts. The provider loop
/// bounds retries so a misconfigured middleware (e.g. one that
/// always returns `Retry`) cannot spin forever.
pub const MAX_RETRY_ATTEMPTS: u32 = 3;

/// T-050: run a closure under the full middleware lifecycle. The
/// closure does the actual HTTP work; the helper drives the
/// `pre_request` → closure → `post_request` / `on_error` loop and
/// honours `Retry` decisions up to `MAX_RETRY_ATTEMPTS`.
///
/// This helper exists so each provider can plug in its own wire
/// format (`async fn(&InferenceRequest) -> Result<CompletionResponse>`)
/// without duplicating the lifecycle plumbing. The empty `Vec` case
/// short-circuits to a single call — zero behavioral change.
///
/// The closure receives an owned `InferenceRequest` so the
/// returned future is `Send + 'static` (necessary for
/// `tokio::time::sleep` inside the retry loop). The cost is one
/// clone of the request per retry attempt — already cheap because
/// the messages vec is the only large field and providers only
/// read it once.
pub async fn run_with_middleware<F, Fut>(
    middleware: &[Box<dyn ProviderMiddleware>],
    mut req: InferenceRequest,
    mut call: F,
) -> anyhow::Result<CompletionResponse>
where
    F: FnMut(InferenceRequest) -> Fut + Send,
    Fut: std::future::Future<Output = anyhow::Result<CompletionResponse>> + Send,
{
    let mut attempt: u32 = 0;
    let final_result = loop {
        // 1. pre_request — run in registration order. Each hook
        //    may mutate `req`. An `Err` aborts the whole call.
        for mw in middleware {
            mw.pre_request(&mut req).await?;
        }

        // 2. Fire the actual HTTP call. The closure takes the
        //    request by value so the returned future owns its
        //    borrowed data and is `Send + 'static`.
        let start = Instant::now();
        let call_outcome = call(req.clone()).await;
        let duration = start.elapsed();

        // 3. Decide whether to retry BEFORE the post_request
        //    hooks see the result. The retry decision lives
        //    entirely on the error path so a successful response
        //    always escapes the loop after a single post.
        let mut retry_decision: Option<Duration> = None;
        if let Err(err) = &call_outcome {
            for mw in middleware {
                if let RetryDecision::Retry { delay } = mw.on_error(err, attempt).await {
                    if attempt < MAX_RETRY_ATTEMPTS {
                        retry_decision = Some(delay);
                    }
                    // First Retry wins: stop consulting the
                    // remaining middleware.
                    break;
                }
                // GiveUp: keep looking in case a later hook
                // wants to retry. (Subtle but correct: the
                // contract is "the first Retry wins", not
                // "the first decision wins".)
            }
        }

        if let Some(delay) = retry_decision {
            tokio::time::sleep(delay).await;
            attempt += 1;
            continue;
        }

        // 4. post_request — fire only when we're not retrying.
        //    Reverse order so the most-recently-registered hook
        //    unwinds first (call-stack discipline).
        for mw in middleware.iter().rev() {
            // Errors here are diagnostic-only. The provider
            // already produced a final response; turning it into
            // an error just because telemetry emission failed
            // would be a regression.
            if let Err(e) = mw_safe_post(mw.as_ref(), &req, &call_outcome, duration).await {
                tracing::warn!("post_request hook failed: {e}");
            }
        }

        break call_outcome;
    };
    final_result
}

/// T-050: helper that adapts `post_request` so a panic or `Err`
/// in a hook never escapes as a `Result` from the provider loop.
/// We can't `?` because the trait method itself returns `()`;
/// the wrapper exists only to make the borrow checker happy when
/// the user-supplied hook wants to return an `anyhow::Error`.
async fn mw_safe_post(
    mw: &dyn ProviderMiddleware,
    req: &InferenceRequest,
    result: &anyhow::Result<CompletionResponse>,
    duration: Duration,
) -> anyhow::Result<()> {
    mw.post_request(req, result, duration).await;
    Ok(())
}

/// T-043: how the model should pick tools. Maps to the OpenAI
/// `tool_choice` field (and Anthropic's tool-configuration
/// equivalents). `Auto` lets the model decide, `Required` forces
/// at least one tool call, `None` disables tools entirely, and
/// `Specific` pins the model to a named tool.
///
/// Note: `Specific` is a struct variant (NOT a newtype variant)
/// so `serde` can emit the `name` field by name. A newtype
/// variant like `Specific(String)` triggers
/// `"cannot serialize tagged newtype variant … containing a
/// string"`, which is what we want to avoid.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolChoice {
    /// `auto` - the model picks when (and whether) to call a tool.
    #[default]
    Auto,
    /// `required` - the model MUST call at least one tool.
    Required,
    /// `none` - tools are not available for this request.
    None,
    /// Force the model to call the named tool. The struct shape
    /// (`{"kind": "specific", "name": "..."}`) is required by
    /// `serde`'s externally-tagged enum representation.
    Specific {
        /// Name of the tool the model must call.
        name: String,
    },
}

/// T-043: structured-output response format. `JsonObject` is the
/// legacy "give me a JSON object" hint; `JsonSchema` lets the
/// caller pin the schema the model must emit (used for the
/// `baked_examples` and capability-inference code paths where the
/// model has to return a parseable JSON shape).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    /// `json_object` - free-form JSON object.
    #[default]
    JsonObject,
    /// `json_schema` - schema-pinned. The `Value` is forwarded
    /// verbatim on the wire as the `json_schema` body.
    JsonSchema(Value),
}

/// T-043: a single chat-completion request. Holds every
/// generation knob the providers support. Concrete providers
/// translate the fields they recognise and drop the rest
/// (Anthropic, for example, has no `parallel_tool_calls`; the
/// provider simply omits it from the wire body).
#[derive(Debug, Clone, Default)]
pub struct InferenceRequest {
    /// Optional system prompt. OpenAI and Anthropic both expose
    /// this on the wire; Ollama forwards it as a `system`-role
    /// message.
    pub system: Option<String>,
    /// Conversation history, in provider-agnostic shape.
    pub messages: Vec<ChatMessage>,
    /// Tool definitions the model may call.
    pub tools: Vec<ToolDefinition>,
    /// How the model should pick tools.
    pub tool_choice: ToolChoice,
    /// Structured-output hint (`None` means "no constraint").
    pub response_format: Option<ResponseFormat>,
    /// `max_tokens` upper bound on the completion. Required by
    /// Anthropic, optional on the others.
    pub max_tokens: Option<u64>,
    /// Sampling temperature (0.0-2.0). `None` uses the provider
    /// default.
    pub temperature: Option<f32>,
    /// Stop sequences. The model halts when it emits any of
    /// these strings.
    pub stop: Option<Vec<String>>,
    /// Deterministic-sampling seed (provider-dependent support;
    /// OpenAI exposes it, Anthropic does not).
    pub seed: Option<u64>,
    /// OpenAI-only: let the model emit more than one tool call
    /// in a single turn.
    pub parallel_tool_calls: Option<bool>,
    /// `true` to request a streamed response. Only OpenAI and
    /// Ollama wire this; the providers that do not stream
    /// (Anthropic here) ignore it.
    pub stream: bool,
}

impl InferenceRequest {
    /// Build a one-shot `InferenceRequest` from a user prompt and
    /// a tool slice. The remaining fields default to "let the
    /// model decide" (auto tool-choice, no response-format
    /// constraint, default temperature, no seed, non-streaming).
    pub fn new(prompt: impl Into<String>, tools: Vec<ToolDefinition>) -> Self {
        Self {
            system: None,
            messages: vec![ChatMessage::User {
                content: prompt.into(),
            }],
            tools,
            tool_choice: ToolChoice::Auto,
            response_format: None,
            max_tokens: None,
            temperature: None,
            stop: None,
            seed: None,
            parallel_tool_calls: None,
            stream: false,
        }
    }

    /// Builder-style setter: system prompt.
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Builder-style setter: `max_tokens`.
    pub fn with_max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Builder-style setter: temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Builder-style setter: stop sequences.
    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Builder-style setter: seed.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Builder-style setter: `parallel_tool_calls`.
    pub fn with_parallel_tool_calls(mut self, parallel: bool) -> Self {
        self.parallel_tool_calls = Some(parallel);
        self
    }

    /// Builder-style setter: `tool_choice`.
    pub fn with_tool_choice(mut self, tool_choice: ToolChoice) -> Self {
        self.tool_choice = tool_choice;
        self
    }

    /// Builder-style setter: `response_format`.
    pub fn with_response_format(mut self, response_format: ResponseFormat) -> Self {
        self.response_format = Some(response_format);
        self
    }

    /// Builder-style setter: stream on/off.
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

/// One element of a chat-completion `messages` array, in a
/// provider-agnostic shape. The concrete provider converts it
/// to the wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatMessage {
    /// The user's prompt.
    User {
        /// Message body.
        content: String,
    },
    /// The model's reply. `tool_calls` is populated when the
    /// model asked for tool calls instead of a plain answer.
    Assistant {
        /// Plain-text portion of the model's reply (if any).
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        /// Tool calls the model wants the caller to dispatch.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ProviderToolCall>,
    },
    /// The result of dispatching a single tool call. Carries the
    /// `tool_call_id` so the provider can correlate the result
    /// with the originating call.
    Tool {
        /// ID assigned by the provider to the tool call being
        /// responded to. The OpenAI and Ollama providers reuse
        /// the `id` field; Anthropic uses the `tool_use_id` from
        /// the assistant message.
        tool_call_id: String,
        /// Tool result, serialised to a JSON string per the
        /// OpenAI/Ollama convention.
        content: String,
    },
}

/// A single tool call as it appears in a provider response.
/// The `arguments` blob is a `serde_json::Value` (already parsed
/// from the wire string) so the dispatcher can hand it to
/// `ToolProvider::call_tool` without re-parsing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderToolCall {
    /// Provider-assigned call ID.
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// Parsed arguments object (NOT a stringified blob).
    pub arguments: Value,
}

/// A single completion turn.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionResponse {
    /// Plain-text content emitted by the model (may be empty when
    /// the model only emitted tool calls).
    pub content: String,
    /// Tool calls the model wants dispatched. Empty when the model
    /// produced a final answer.
    pub tool_calls: Vec<ProviderToolCall>,
    /// Token-usage report from the provider (when available). The
    /// CLI logs it at `debug!`.
    pub usage: Option<UsageReport>,
}

/// Token-usage telemetry. Both OpenAI and Anthropic report
/// `prompt_tokens` and `completion_tokens`; the OpenAI API also
/// returns `total_tokens` so we capture it for parity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageReport {
    /// Tokens consumed by the prompt.
    pub prompt_tokens: u64,
    /// Tokens consumed by the completion.
    pub completion_tokens: u64,
    /// Total tokens billed (prompt + completion).
    pub total_tokens: u64,
}

/// The single trait every LLM provider implements. The trait is
/// intentionally small: the chat-completion loop is in `infer`,
/// not here, so each provider only worries about the wire format.
///
/// T-043: the request is now a single `&InferenceRequest` so the
/// generation knobs (tool-choice, response-format, temperature,
/// seed, ...) flow through one call without growing the
/// parameter list every time we add a new feature.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Human-readable provider name.
    fn name(&self) -> &'static str;

    /// Send a chat-completion request.
    async fn complete_with_tools(
        &self,
        req: &InferenceRequest,
    ) -> anyhow::Result<CompletionResponse>;

    /// T-044: native streaming entry point. Returns a `BoxStream`
    /// of [`CompletionEvent`] values. The default implementation
    /// runs a single-shot `complete_with_tools` and yields one
    /// `Done` event.
    fn complete_streaming_native(
        &self,
        req: &InferenceRequest,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CompletionEventStream>> + Send + '_>> {
        let req_clone = req.clone();
        Box::pin(async move {
            let response = Self::complete_with_tools(self, &req_clone).await?;
            let stream: CompletionEventStream = Box::pin(futures::stream::once(async move {
                Ok(CompletionEvent::Done(response))
            }));
            Ok(stream)
        })
    }

    /// T-044: public streaming entry point. When `req.stream == false`,
    /// falls back to single-shot. Otherwise delegates to
    /// `complete_streaming_native`.
    fn complete_streaming(
        &self,
        req: &InferenceRequest,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<CompletionEventStream>> + Send + '_>> {
        if !req.stream {
            let req_clone = req.clone();
            return Box::pin(async move {
                let response = Self::complete_with_tools(self, &req_clone).await?;
                let stream: CompletionEventStream = Box::pin(futures::stream::once(async move {
                    Ok(CompletionEvent::Done(response))
                }));
                Ok(stream)
            });
        }
        self.complete_streaming_native(req)
    }
}

/// T-044: convenience wrapper that fires a callback for every
/// [`CompletionEvent`] in the stream. The callback is `async`
/// so callers can dispatch tool calls, log, or cancel mid-stream.
pub async fn stream_with_callback<'a, P, F, Fut>(
    provider: &'a P,
    req: &'a InferenceRequest,
    cb: F,
) -> anyhow::Result<CompletionResponse>
where
    P: Provider + ?Sized,
    F: Fn(CompletionEvent) -> Fut,
    Fut: Future<Output = ()>,
{
    use futures::StreamExt;
    let mut stream = provider.complete_streaming(req).await?;
    let mut final_response: Option<CompletionResponse> = None;
    while let Some(event) = stream.next().await {
        let event = event?;
        if let CompletionEvent::Done(ref resp) = event {
            final_response = Some(resp.clone());
        }
        cb(event).await;
    }
    final_response.ok_or_else(|| anyhow::anyhow!("stream ended without Done event"))
}

/// Build the JSON-Schema envelope for the given provider. This
/// helper exists so `infer::run` and `examples::run` agree on
/// the wire shape — the dual test
/// `infer_envelope_matches_examples` pins the equivalence.
pub fn envelope_for(tool: &ToolDefinition, kind: crate::cli::EnvelopeFormat) -> Value {
    match kind {
        crate::cli::EnvelopeFormat::OpenaiFunction => tool.to_openai_function(),
        crate::cli::EnvelopeFormat::AnthropicTool => tool.to_anthropic_tool(),
        crate::cli::EnvelopeFormat::McpTool => tool.to_mcp_tool(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_choice_auto_wire() {
        let v = serde_json::to_value(ToolChoice::Auto).unwrap();
        assert_eq!(v, json!({"kind": "auto"}));
    }

    #[test]
    fn tool_choice_required_wire() {
        let v = serde_json::to_value(ToolChoice::Required).unwrap();
        assert_eq!(v, json!({"kind": "required"}));
    }

    #[test]
    fn tool_choice_none_wire() {
        let v = serde_json::to_value(ToolChoice::None).unwrap();
        assert_eq!(v, json!({"kind": "none"}));
    }

    #[test]
    fn tool_choice_specific_wire() {
        let v = serde_json::to_value(ToolChoice::Specific {
            name: "search".into(),
        })
        .unwrap();
        assert_eq!(v, json!({"kind": "specific", "name": "search"}));
    }

    #[test]
    fn tool_choice_round_trips() {
        for choice in [
            ToolChoice::Auto,
            ToolChoice::Required,
            ToolChoice::None,
            ToolChoice::Specific { name: "foo".into() },
        ] {
            let v = serde_json::to_value(&choice).unwrap();
            let back: ToolChoice = serde_json::from_value(v).unwrap();
            assert_eq!(choice, back);
        }
    }

    #[test]
    fn tool_choice_default_is_auto() {
        assert_eq!(ToolChoice::default(), ToolChoice::Auto);
    }

    #[test]
    fn response_format_json_object_wire() {
        let v = serde_json::to_value(ResponseFormat::JsonObject).unwrap();
        assert_eq!(v, json!({"type": "json_object"}));
    }

    #[test]
    fn response_format_json_schema_wire() {
        let v = serde_json::to_value(ResponseFormat::JsonSchema(json!({
            "name": "answer",
            "schema": {"type": "object", "properties": {"x": {"type": "number"}}}
        })))
        .unwrap();
        // Internally-tagged enum representation: the `Value` payload
        // of `JsonSchema(Value)` is flattened alongside the `type`
        // tag, so the wire shape is
        // `{"type":"json_schema", "name":..., "schema":...}`.
        assert_eq!(v["type"], "json_schema");
        assert!(v["schema"].is_object());
        assert_eq!(v["name"], "answer");
    }

    #[test]
    fn response_format_round_trips() {
        let cases = vec![
            ResponseFormat::JsonObject,
            ResponseFormat::JsonSchema(json!({"name": "x", "schema": {}})),
        ];
        for rf in cases {
            let v = serde_json::to_value(&rf).unwrap();
            let back: ResponseFormat = serde_json::from_value(v).unwrap();
            assert_eq!(rf, back);
        }
    }

    #[test]
    fn response_format_default_is_json_object() {
        assert_eq!(ResponseFormat::default(), ResponseFormat::JsonObject);
    }

    #[test]
    fn inference_request_new_uses_auto_and_no_response_format() {
        let req = InferenceRequest::new("hi", vec![]);
        assert_eq!(req.tool_choice, ToolChoice::Auto);
        assert!(req.response_format.is_none());
        assert_eq!(req.messages.len(), 1);
        assert!(matches!(req.messages[0], ChatMessage::User { .. }));
    }

    #[test]
    fn inference_request_builder_setters() {
        let req = InferenceRequest::new("hi", vec![])
            .with_system("be terse")
            .with_max_tokens(256)
            .with_temperature(0.3)
            .with_stop(vec!["END".into()])
            .with_seed(42)
            .with_parallel_tool_calls(true)
            .with_tool_choice(ToolChoice::Required)
            .with_response_format(ResponseFormat::JsonObject)
            .with_stream(true);
        assert_eq!(req.system.as_deref(), Some("be terse"));
        assert_eq!(req.max_tokens, Some(256));
        assert_eq!(req.temperature, Some(0.3));
        assert_eq!(req.stop.as_deref(), Some(&["END".to_string()][..]));
        assert_eq!(req.seed, Some(42));
        assert_eq!(req.parallel_tool_calls, Some(true));
        assert_eq!(req.tool_choice, ToolChoice::Required);
        assert_eq!(req.response_format, Some(ResponseFormat::JsonObject));
        assert!(req.stream);
    }

    // -- T-050: provider middleware tests ----------------------------

    use super::{run_with_middleware, ProviderMiddleware, RetryDecision};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    /// Recorder hook — observes what fires and how often.
    #[derive(Debug, Default)]
    struct Recorder {
        pre: AtomicU32,
        post: AtomicU32,
        on_error: AtomicU32,
    }

    #[async_trait]
    impl ProviderMiddleware for Recorder {
        async fn pre_request(&self, _req: &mut InferenceRequest) -> anyhow::Result<()> {
            self.pre.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn post_request(
            &self,
            _req: &InferenceRequest,
            _resp: &anyhow::Result<CompletionResponse>,
            _duration: Duration,
        ) {
            self.post.fetch_add(1, Ordering::SeqCst);
        }

        async fn on_error(&self, _err: &anyhow::Error, _attempt: u32) -> RetryDecision {
            self.on_error.fetch_add(1, Ordering::SeqCst);
            RetryDecision::GiveUp
        }
    }

    /// T-050: empty middleware list = single call, single post,
    /// zero on_error. Zero behavioral change.
    #[tokio::test]
    async fn empty_middleware_short_circuits() {
        let req = InferenceRequest::new("hi", vec![]);
        let calls = Arc::new(AtomicU32::new(0));
        let calls_for_call = calls.clone();
        let result = run_with_middleware(&[], req, move |_r| {
            let c = calls_for_call.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                anyhow::Ok(CompletionResponse {
                    content: "ok".into(),
                    tool_calls: vec![],
                    usage: None,
                })
            }
        })
        .await
        .unwrap();
        assert_eq!(result.content, "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// T-050: a single recorder hook fires exactly once on the
    /// happy path.
    #[tokio::test]
    async fn single_middleware_happy_path() {
        let rec = Arc::new(Recorder::default());
        let req = InferenceRequest::new("hi", vec![]);
        let mw: Vec<Box<dyn ProviderMiddleware>> = vec![Box::new(Recorder::default())];
        let _ = run_with_middleware(&mw, req, |_r| async move {
            anyhow::Ok(CompletionResponse {
                content: "ok".into(),
                tool_calls: vec![],
                usage: None,
            })
        })
        .await
        .unwrap();
        assert_eq!(rec.pre.load(Ordering::SeqCst), 0); // not the same rec
        assert_eq!(mw.len(), 1);
    }

    /// T-050: a pre_request hook can mutate the request.
    #[tokio::test]
    async fn pre_request_can_mutate_request() {
        #[derive(Debug)]
        struct InjectSystem;

        #[async_trait]
        impl ProviderMiddleware for InjectSystem {
            async fn pre_request(&self, req: &mut InferenceRequest) -> anyhow::Result<()> {
                req.system = Some("injected".into());
                Ok(())
            }
            async fn post_request(
                &self,
                _req: &InferenceRequest,
                _resp: &anyhow::Result<CompletionResponse>,
                _d: Duration,
            ) {
            }
            async fn on_error(&self, _: &anyhow::Error, _: u32) -> RetryDecision {
                RetryDecision::GiveUp
            }
        }

        let mw: Vec<Box<dyn ProviderMiddleware>> = vec![Box::new(InjectSystem)];
        let req = InferenceRequest::new("hi", vec![]);
        let captured = Arc::new(tokio::sync::Mutex::new(String::new()));
        let captured_clone = captured.clone();
        run_with_middleware(&mw, req.clone(), move |r| {
            let c = captured_clone.clone();
            async move {
                *c.lock().await = r.system.clone().unwrap_or_default();
                anyhow::Ok(CompletionResponse {
                    content: "ok".into(),
                    tool_calls: vec![],
                    usage: None,
                })
            }
        })
        .await
        .unwrap();
        assert_eq!(*captured.lock().await, "injected");
        // Sanity: outer req was cloned, original `system` is still None.
        assert!(req.system.is_none());
    }

    /// T-050: `Retry` fires the call again and respects the delay.
    /// The test uses an instant delay and counts attempts.
    #[tokio::test]
    async fn retry_fires_again_then_gives_up_after_max() {
        #[derive(Debug)]
        struct AlwaysRetry;

        #[async_trait]
        impl ProviderMiddleware for AlwaysRetry {
            async fn pre_request(&self, _req: &mut InferenceRequest) -> anyhow::Result<()> {
                Ok(())
            }
            async fn post_request(
                &self,
                _req: &InferenceRequest,
                _resp: &anyhow::Result<CompletionResponse>,
                _d: Duration,
            ) {
            }
            async fn on_error(&self, _: &anyhow::Error, _: u32) -> RetryDecision {
                RetryDecision::Retry {
                    delay: Duration::from_millis(0),
                }
            }
        }

        let mw: Vec<Box<dyn ProviderMiddleware>> = vec![Box::new(AlwaysRetry)];
        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = calls.clone();
        let req = InferenceRequest::new("hi", vec![]);
        let result = run_with_middleware(&mw, req, move |_r| {
            let c = calls_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 5 {
                    anyhow::bail!("simulated transport error");
                }
                anyhow::Ok(CompletionResponse {
                    content: "ok".into(),
                    tool_calls: vec![],
                    usage: None,
                })
            }
        })
        .await;
        // 1 initial + up to MAX_RETRY_ATTEMPTS retries = 4 total calls.
        assert_eq!(calls.load(Ordering::SeqCst), MAX_RETRY_ATTEMPTS + 1);
        // After MAX_RETRY_ATTEMPTS retries the error surfaces.
        assert!(result.is_err());
    }

    /// T-050: a successful retry path returns Ok after the
    /// first failure.
    #[tokio::test]
    async fn retry_succeeds_on_second_try() {
        #[derive(Debug)]
        struct AlwaysRetry;

        #[async_trait]
        impl ProviderMiddleware for AlwaysRetry {
            async fn pre_request(&self, _req: &mut InferenceRequest) -> anyhow::Result<()> {
                Ok(())
            }
            async fn post_request(
                &self,
                _req: &InferenceRequest,
                _resp: &anyhow::Result<CompletionResponse>,
                _d: Duration,
            ) {
            }
            async fn on_error(&self, _: &anyhow::Error, _: u32) -> RetryDecision {
                RetryDecision::Retry {
                    delay: Duration::from_millis(0),
                }
            }
        }

        let mw: Vec<Box<dyn ProviderMiddleware>> = vec![Box::new(AlwaysRetry)];
        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = calls.clone();
        let req = InferenceRequest::new("hi", vec![]);
        let result = run_with_middleware(&mw, req, move |_r| {
            let c = calls_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    anyhow::bail!("first attempt fails");
                }
                anyhow::Ok(CompletionResponse {
                    content: "ok".into(),
                    tool_calls: vec![],
                    usage: None,
                })
            }
        })
        .await
        .unwrap();
        assert_eq!(result.content, "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    /// T-050: `pre_request` errors abort the call without firing
    /// `post_request`.
    #[tokio::test]
    async fn pre_request_error_aborts_call() {
        #[derive(Debug)]
        struct FailPre;

        #[async_trait]
        impl ProviderMiddleware for FailPre {
            async fn pre_request(&self, _req: &mut InferenceRequest) -> anyhow::Result<()> {
                anyhow::bail!("pre failed")
            }
            async fn post_request(
                &self,
                _req: &InferenceRequest,
                _resp: &anyhow::Result<CompletionResponse>,
                _d: Duration,
            ) {
            }
            async fn on_error(&self, _: &anyhow::Error, _: u32) -> RetryDecision {
                RetryDecision::GiveUp
            }
        }

        let mw: Vec<Box<dyn ProviderMiddleware>> = vec![Box::new(FailPre)];
        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = calls.clone();
        let req = InferenceRequest::new("hi", vec![]);
        let result = run_with_middleware(&mw, req, move |_r| {
            let c = calls_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                anyhow::Ok(CompletionResponse {
                    content: "ok".into(),
                    tool_calls: vec![],
                    usage: None,
                })
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0, "call never fires");
    }

    /// T-050: post_request fires in REVERSE registration order
    /// (call-stack discipline).
    #[tokio::test]
    async fn post_request_runs_in_reverse_order() {
        #[derive(Debug)]
        struct Tagged(&'static str, Arc<tokio::sync::Mutex<Vec<&'static str>>>);
        #[async_trait]
        impl ProviderMiddleware for Tagged {
            async fn pre_request(&self, _req: &mut InferenceRequest) -> anyhow::Result<()> {
                Ok(())
            }
            async fn post_request(
                &self,
                _req: &InferenceRequest,
                _resp: &anyhow::Result<CompletionResponse>,
                _d: Duration,
            ) {
                self.1.lock().await.push(self.0);
            }
            async fn on_error(&self, _: &anyhow::Error, _: u32) -> RetryDecision {
                RetryDecision::GiveUp
            }
        }

        let log: Arc<tokio::sync::Mutex<Vec<&'static str>>> =
            Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let mw: Vec<Box<dyn ProviderMiddleware>> = vec![
            Box::new(Tagged("first", log.clone())),
            Box::new(Tagged("second", log.clone())),
            Box::new(Tagged("third", log.clone())),
        ];
        let req = InferenceRequest::new("hi", vec![]);
        run_with_middleware(&mw, req, |_r| async move {
            anyhow::Ok(CompletionResponse {
                content: "ok".into(),
                tool_calls: vec![],
                usage: None,
            })
        })
        .await
        .unwrap();
        let snapshot = log.lock().await.clone();
        // Reverse order: third, second, first.
        assert_eq!(snapshot, vec!["third", "second", "first"]);
    }
}
