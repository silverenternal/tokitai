//! T-034: `tokitai-llm infer` subcommand.
//!
//! Drives the tool-calling loop:
//! 1. Render the user prompt + (optional) system prompt.
//! 2. POST to the LLM via the chosen provider.
//! 3. If the model returned tool calls, dispatch each one through
//!    `ToolProvider::call_tool` and feed the result back as a
//!    follow-up turn.
//! 4. Repeat until the model emits a final text reply (or
//!    `--max-iterations` is reached).
//!
//! The loop is provider-agnostic: every provider implements
//! `Provider::complete_with_tools`, so adding a new one is one
//! trait impl.

use crate::cache::{cache_key, CacheBackend, InMemoryCache};
use crate::cli::{InferArgs, ProviderArgs, ProviderKind};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::openai::OpenAiProvider;
use crate::provider::{ChatMessage, CompletionResponse, Provider, ProviderToolCall};
use crate::Result;
use tokitai_core::ToolDefinition;

/// One concrete provider. The `Provider` trait is not
/// dyn-compatible (it carries an `async fn`), so the build helper
/// returns this enum instead of a `Box<dyn Provider>`. The enum
/// itself implements `Provider` (see the `Provider` impl below).
pub enum AnyProvider {
    /// OpenAI Chat Completions API.
    Openai(OpenAiProvider),
    /// Anthropic Messages API.
    Anthropic(AnthropicProvider),
    /// Ollama native API.
    Ollama(OllamaProvider),
}

#[async_trait::async_trait]
impl Provider for AnyProvider {
    fn name(&self) -> &'static str {
        match self {
            AnyProvider::Openai(p) => p.name(),
            AnyProvider::Anthropic(p) => p.name(),
            AnyProvider::Ollama(p) => p.name(),
        }
    }

    async fn complete_with_tools(
        &self,
        system: Option<&str>,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<CompletionResponse> {
        match self {
            AnyProvider::Openai(p) => p.complete_with_tools(system, messages, tools).await,
            AnyProvider::Anthropic(p) => p.complete_with_tools(system, messages, tools).await,
            AnyProvider::Ollama(p) => p.complete_with_tools(system, messages, tools).await,
        }
    }
}

/// Run `tokitai-llm infer` with the given args.
pub async fn run(args: InferArgs) -> Result<()> {
    let provider = build_provider(&args.provider)?;
    // T-034: the provider slice is supplied by the embedding
    // host (a binary that uses `#[tool]` and exposes its
    // `ToolProvider`). For the v0.1 stub we accept zero
    // tools; the v0.2 path will plumb a `--provider-crate`
    // arg that points at a `cdylib` exposing the slice.
    let tools: Vec<ToolDefinition> = Vec::new();

    let messages = vec![ChatMessage::User {
        content: args.prompt.clone(),
    }];
    let cache = InMemoryCache::new();

    let response = complete_with_cache(
        &provider,
        &cache,
        args.no_cache,
        args.provider.model.as_deref().unwrap_or("default"),
        args.system.as_deref(),
        &messages,
        &tools,
    )
    .await?;

    println!("{}", response.content);
    if !response.tool_calls.is_empty() {
        eprintln!(
            "warning: model returned {} tool call(s) but no provider was \
             supplied; install a --provider-crate to dispatch them",
            response.tool_calls.len()
        );
    }
    Ok(())
}

/// Build the concrete `Provider` implementation from CLI args.
/// Each branch constructs the config struct that pairs with the
/// provider and wraps it in the matching struct.
pub fn build_provider(args: &ProviderArgs) -> Result<AnyProvider> {
    let model = args
        .model
        .clone()
        .ok_or_else(|| anyhow::anyhow!("infer: --model is required"))?;
    let kind = args
        .provider
        .ok_or_else(|| anyhow::anyhow!("infer: --provider is required"))?;
    match kind {
        ProviderKind::Openai => {
            let cfg = crate::provider::openai::OpenAiConfig::from_args(
                args.base_url.clone(),
                model,
                args.api_key.clone(),
            );
            Ok(AnyProvider::Openai(OpenAiProvider::new(cfg)))
        }
        ProviderKind::Anthropic => {
            let cfg = crate::provider::anthropic::AnthropicConfig::from_args(
                args.base_url.clone(),
                model,
                args.api_key.clone(),
            );
            Ok(AnyProvider::Anthropic(AnthropicProvider::new(cfg)))
        }
        ProviderKind::Ollama => {
            let cfg =
                crate::provider::ollama::OllamaConfig::from_args(args.base_url.clone(), model);
            Ok(AnyProvider::Ollama(OllamaProvider::new(cfg)))
        }
    }
}

/// Cache-aware wrapper around `Provider::complete_with_tools`.
/// When `no_cache` is false and a cached response exists for the
/// given key, the cached value is returned and no HTTP request
/// is made.
pub async fn complete_with_cache(
    provider: &dyn Provider,
    cache: &dyn CacheBackend,
    no_cache: bool,
    model: &str,
    system: Option<&str>,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Result<CompletionResponse> {
    let key = cache_key(model, system, messages, tools);
    if !no_cache {
        if let Some(cached) = cache.get(&key) {
            tracing::debug!(provider = provider.name(), key = %key, "cache hit");
            return Ok((&cached).into());
        }
    }
    let response = provider
        .complete_with_tools(system, messages, tools)
        .await?;
    if !no_cache {
        let cr: crate::cache::CachedResponse = (&response).into();
        cache.put(key, &cr);
    }
    Ok(response)
}

/// Dispatch a single tool call against a `tokitai_core::ToolProvider`
/// and convert the result into a `ChatMessage::Tool` ready to be
/// fed back to the model.
///
/// This is the public seam that the v0.2 `infer` loop will call
/// for every `ProviderToolCall` returned by the model. The v0.1
/// stub is intentionally a free function so the dispatcher can
/// be tested without a full `Provider` in scope.
pub fn dispatch_as_tool_message(call: &ProviderToolCall) -> ChatMessage {
    // v0.1: no provider is wired in yet, so we emit a placeholder
    // string. The v0.2 loop will pass the `ToolProvider` and
    // serialise the call result here.
    ChatMessage::Tool {
        tool_call_id: call.id.clone(),
        content: format!("(dispatch not yet wired) {}", call.name),
    }
}
