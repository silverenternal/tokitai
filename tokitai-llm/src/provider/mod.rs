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
use tokitai_core::ToolDefinition;

pub mod anthropic;
pub mod ollama;
pub mod openai;

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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderToolCall {
    /// Provider-assigned call ID.
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// Parsed arguments object (NOT a stringified blob).
    pub arguments: Value,
}

/// A single completion turn.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Human-readable provider name (e.g. `"openai"`, `"anthropic"`,
    /// `"ollama"`). Used in logs and cache keys.
    fn name(&self) -> &'static str;

    /// Send a chat-completion request. The provider is responsible
    /// for translating `req` into its native wire format and
    /// parsing the response back into [`CompletionResponse`].
    async fn complete_with_tools(
        &self,
        req: &InferenceRequest,
    ) -> anyhow::Result<CompletionResponse>;
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
}
