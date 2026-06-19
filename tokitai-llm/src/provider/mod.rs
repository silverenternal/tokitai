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

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokitai_core::ToolDefinition;

pub mod anthropic;
pub mod ollama;
pub mod openai;

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
#[async_trait]
pub trait Provider: Send + Sync {
    /// Human-readable provider name (e.g. `"openai"`, `"anthropic"`,
    /// `"ollama"`). Used in logs and cache keys.
    fn name(&self) -> &'static str;

    /// Send a chat-completion request. The provider is responsible
    /// for converting `messages` and `tools` into its native wire
    /// format and parsing the response back into
    /// [`CompletionResponse`].
    async fn complete_with_tools(
        &self,
        system: Option<&str>,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
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
