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
//!
//! T-043: the per-iteration request is now built into an
//! `InferenceRequest` struct so the same shape works for every
//! provider and every call site (verify / examples /
//! infer-capabilities / infer).

use crate::cache::{cache_key_v2, CacheBackend, InMemoryCache, ToolCache};
use crate::cli::{InferArgs, ProviderArgs, ProviderKind};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::openai::OpenAiProvider;
use crate::provider::{CompletionResponse, InferenceRequest, Provider};
use crate::Result;
use serde_json::Value;
use tokitai_core::{ToolCaller, ToolDefinition};

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
        req: &InferenceRequest,
    ) -> anyhow::Result<CompletionResponse> {
        match self {
            AnyProvider::Openai(p) => p.complete_with_tools(req).await,
            AnyProvider::Anthropic(p) => p.complete_with_tools(req).await,
            AnyProvider::Ollama(p) => p.complete_with_tools(req).await,
        }
    }
}

/// Run `tokitai-llm infer` with the given args.
///
/// `tool_cache` is optional. When `Some`, every tool dispatch inside
/// the run is routed through `ToolCache::get_or_compute` so the
/// self-consistency multi-sample path collapses repeated `(name,
/// args)` lookups into a single `ToolProvider::call_tool`
/// invocation. When `None`, every call hits the provider
/// unconditionally — same behaviour as before T-047.
pub async fn run(args: InferArgs, tool_cache: Option<ToolCache>) -> Result<()> {
    let provider = build_provider(&args.provider)?;
    // T-034: the provider slice is supplied by the embedding
    // host (a binary that uses `#[tool]` and exposes its
    // `ToolProvider`). For the v0.1 stub we accept zero
    // tools; the v0.2 path will plumb a `--provider-crate`
    // arg that points at a `cdylib` exposing the slice.
    let tools: Vec<ToolDefinition> = Vec::new();

    // T-043: build the request up-front so the CLI args
    // (system, tool_choice, response_format, temperature, seed,
    // stream) flow through one shape.
    let mut req = InferenceRequest::new(args.prompt.clone(), tools.clone());
    if let Some(sys) = args.system.as_deref() {
        req.system = Some(sys.to_string());
    }
    if let Some(max) = args.provider.max_tokens {
        req.max_tokens = Some(max);
    }
    if let Some(temp) = args.temperature {
        req.temperature = Some(temp);
    }
    if let Some(seed) = args.seed {
        req.seed = Some(seed);
    }
    if let Some(tc) = args.tool_choice {
        req.tool_choice = tc.into();
    }
    if let Some(rf) = args.response_format {
        req.response_format = Some(rf.into());
    }
    req.stream = !args.no_stream;
    // Messages were set by `InferenceRequest::new`; nothing else
    // to do for the v0.1 stub.

    let cache = InMemoryCache::new();

    // `build_provider` already validated that `model` is set
    // (it returns `Err` otherwise), so unwrapping here is safe
    // and removes the silent `"default"` fallback.
    let model = args
        .provider
        .model
        .as_deref()
        .expect("model validated by build_provider");

    let response = complete_with_cache(&provider, &cache, args.no_cache, model, &req).await?;

    println!("{}", response.content);
    if !response.tool_calls.is_empty() {
        // T-047: when a cache is configured and an embedded provider
        // is wired in (v0.2), the dispatch path will go through
        // `dispatch_with_cache`. For the v0.1 stub we just warn.
        let _ = tool_cache; // silence unused-when-no-provider
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
pub(crate) fn build_provider(args: &ProviderArgs) -> Result<AnyProvider> {
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
                args.max_tokens,
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
///
/// T-043: the request is now an `InferenceRequest`; the cache
/// key is the v2 hash that includes tool_choice,
/// response_format, temperature, stop, and seed.
pub async fn complete_with_cache(
    provider: &dyn Provider,
    cache: &dyn CacheBackend,
    no_cache: bool,
    model: &str,
    req: &InferenceRequest,
) -> Result<CompletionResponse> {
    let key = cache_key_v2(model, req);
    if !no_cache {
        if let Some(cached) = cache.get(&key) {
            tracing::debug!(provider = provider.name(), key = %key, "cache hit");
            return Ok((&cached).into());
        }
    }
    let response = provider.complete_with_tools(req).await?;
    if !no_cache {
        let cr: crate::cache::CachedResponse = (&response).into();
        cache.put(key, &cr);
    }
    Ok(response)
}

/// Dispatch a tool call through `ToolProvider::call_tool`, with an
/// optional `ToolCache` in front. When `tool_cache` is `Some`,
/// repeated calls with the same `(name, args)` reuse the cached
/// result and never reach the provider. When `None`, every call
/// hits the provider unconditionally — the pre-T-047 behaviour.
///
/// The arguments are taken as `&serde_json::Value` because that is
/// what `ProviderToolCall.arguments` already carries, and the cache
/// serialises that exact value into the key.
///
/// `ToolCaller` is `dyn`-compatible (only `&self` methods), so the
/// provider is taken as `&dyn ToolCaller`. The cache helper still
/// accepts `&Value` and forwards errors through `anyhow::Error`.
pub async fn dispatch_with_cache(
    provider: &dyn ToolCaller,
    tool_cache: Option<&ToolCache>,
    name: &str,
    args: &Value,
) -> Result<Value> {
    match tool_cache {
        None => Ok(provider.call_tool(name, args)?),
        Some(cache) => {
            cache
                .get_or_compute(name, args, || async {
                    provider.call_tool(name, args).map_err(anyhow::Error::from)
                })
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::InMemoryCache;
    use crate::cli::ProviderKind;
    use crate::provider::ProviderToolCall;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct MockProvider {
        name: &'static str,
        content: String,
        called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn name(&self) -> &'static str {
            self.name
        }
        async fn complete_with_tools(
            &self,
            _req: &InferenceRequest,
        ) -> anyhow::Result<CompletionResponse> {
            self.called.store(true, Ordering::SeqCst);
            Ok(CompletionResponse {
                content: self.content.clone(),
                tool_calls: vec![],
                usage: None,
            })
        }
    }

    #[test]
    fn build_provider_openai() {
        let args = ProviderArgs {
            provider: Some(ProviderKind::Openai),
            base_url: Some("https://api.example.com".into()),
            model: Some("gpt-4o".into()),
            api_key: Some("k".into()),
            max_tokens: None,
        };
        let p = build_provider(&args).expect("build_provider openai");
        assert!(matches!(p, AnyProvider::Openai(_)));
    }

    #[test]
    fn build_provider_anthropic() {
        let args = ProviderArgs {
            provider: Some(ProviderKind::Anthropic),
            base_url: Some("https://api.example.com".into()),
            model: Some("claude-3-5-sonnet-latest".into()),
            api_key: Some("k".into()),
            max_tokens: Some(1024),
        };
        let p = build_provider(&args).expect("build_provider anthropic");
        assert!(matches!(p, AnyProvider::Anthropic(_)));
    }

    #[test]
    fn build_provider_ollama() {
        let args = ProviderArgs {
            provider: Some(ProviderKind::Ollama),
            base_url: Some("http://localhost:11434".into()),
            model: Some("llama3.1".into()),
            api_key: None,
            max_tokens: None,
        };
        let p = build_provider(&args).expect("build_provider ollama");
        assert!(matches!(p, AnyProvider::Ollama(_)));
    }

    #[test]
    fn build_provider_errors_without_model() {
        let args = ProviderArgs {
            provider: Some(ProviderKind::Openai),
            base_url: None,
            model: None,
            api_key: None,
            max_tokens: None,
        };
        assert!(build_provider(&args).is_err());
    }

    #[test]
    fn build_provider_errors_without_kind() {
        let args = ProviderArgs {
            provider: None,
            base_url: None,
            model: Some("gpt-4o".into()),
            api_key: None,
            max_tokens: None,
        };
        assert!(build_provider(&args).is_err());
    }

    #[test]
    fn any_provider_name_dispatches_correctly() {
        let openai = AnyProvider::Openai(OpenAiProvider::new(
            crate::provider::openai::OpenAiConfig::from_args(
                None,
                "gpt-4o".into(),
                Some("k".into()),
            ),
        ));
        let anthropic = AnyProvider::Anthropic(AnthropicProvider::new(
            crate::provider::anthropic::AnthropicConfig::from_args(
                None,
                "claude-3-5-sonnet-latest".into(),
                Some("k".into()),
                None,
            ),
        ));
        let ollama = AnyProvider::Ollama(OllamaProvider::new(
            crate::provider::ollama::OllamaConfig::from_args(None, "llama3.1".into()),
        ));
        assert_eq!(openai.name(), "openai");
        assert_eq!(anthropic.name(), "anthropic");
        assert_eq!(ollama.name(), "ollama");
    }

    #[tokio::test]
    async fn complete_with_cache_hit_skips_provider() {
        let cache = InMemoryCache::new();
        let called = Arc::new(AtomicBool::new(false));
        let provider = MockProvider {
            name: "mock",
            content: "live".into(),
            called: Arc::clone(&called),
        };

        let req = InferenceRequest::new("hello", vec![]);
        let key = crate::cache::cache_key_v2("mock-model", &req);
        cache.put(
            key,
            &crate::cache::CachedResponse {
                content: "cached".into(),
                tool_calls: vec![],
                usage: None,
            },
        );

        let resp = complete_with_cache(&provider, &cache, false, "mock-model", &req)
            .await
            .expect("complete_with_cache should succeed");
        assert_eq!(resp.content, "cached");
        assert!(
            !called.load(Ordering::SeqCst),
            "provider should not be called when cache hit"
        );
    }

    #[tokio::test]
    async fn complete_with_cache_miss_calls_provider_and_fills_cache() {
        let cache = InMemoryCache::new();
        let called = Arc::new(AtomicBool::new(false));
        let provider = MockProvider {
            name: "mock",
            content: "live".into(),
            called: Arc::clone(&called),
        };

        let req = InferenceRequest::new("hello", vec![]);

        let resp = complete_with_cache(&provider, &cache, false, "mock-model", &req)
            .await
            .expect("complete_with_cache should succeed");
        assert_eq!(resp.content, "live");
        assert!(
            called.load(Ordering::SeqCst),
            "provider must be called on miss"
        );

        let key = crate::cache::cache_key_v2("mock-model", &req);
        let cached = cache.get(&key).expect("cache populated after miss");
        assert_eq!(cached.content, "live");
    }

    #[tokio::test]
    async fn complete_with_cache_no_cache_bypasses_cache() {
        let cache = InMemoryCache::new();
        let called = Arc::new(AtomicBool::new(false));
        let provider = MockProvider {
            name: "mock",
            content: "live".into(),
            called: Arc::clone(&called),
        };

        let req = InferenceRequest::new("hello", vec![]);
        let key = crate::cache::cache_key_v2("mock-model", &req);
        cache.put(
            key.clone(),
            &crate::cache::CachedResponse {
                content: "cached".into(),
                tool_calls: vec![],
                usage: None,
            },
        );

        let resp = complete_with_cache(&provider, &cache, true, "mock-model", &req)
            .await
            .expect("complete_with_cache should succeed");
        assert_eq!(resp.content, "live");
        assert!(
            called.load(Ordering::SeqCst),
            "provider must be called when no_cache=true"
        );
        let cached = cache.get(&key).expect("entry still present");
        assert_eq!(cached.content, "cached");
    }

    #[tokio::test]
    async fn complete_with_cache_propagates_tool_calls() {
        let cache = InMemoryCache::new();
        let called = Arc::new(AtomicBool::new(false));
        let provider = MockProviderToolCalls {
            called: Arc::clone(&called),
        };
        let req = InferenceRequest::new("hi", vec![]);
        let resp = complete_with_cache(&provider, &cache, false, "mock-model", &req)
            .await
            .expect("complete_with_cache should succeed");
        assert_eq!(resp.content, "");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "echo");
        assert_eq!(resp.tool_calls[0].arguments, json!({"x": 1}));

        called.store(false, Ordering::SeqCst);
        let resp2 = complete_with_cache(&provider, &cache, false, "mock-model", &req)
            .await
            .expect("complete_with_cache should succeed");
        assert_eq!(resp2.tool_calls.len(), 1);
        assert_eq!(resp2.tool_calls[0].name, "echo");
        assert!(
            !called.load(Ordering::SeqCst),
            "second call must come from cache"
        );
    }

    struct MockProviderToolCalls {
        called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl Provider for MockProviderToolCalls {
        fn name(&self) -> &'static str {
            "mock"
        }
        async fn complete_with_tools(
            &self,
            _req: &InferenceRequest,
        ) -> anyhow::Result<CompletionResponse> {
            self.called.store(true, Ordering::SeqCst);
            Ok(CompletionResponse {
                content: String::new(),
                tool_calls: vec![ProviderToolCall {
                    id: "call_1".into(),
                    name: "echo".into(),
                    arguments: json!({"x": 1}),
                }],
                usage: None,
            })
        }
    }

    #[test]
    fn provider_trait_dyn_compatible_via_box() {
        let p: Box<dyn Provider> = Box::new(OpenAiProvider::new(
            crate::provider::openai::OpenAiConfig::from_args(
                None,
                "gpt-4o".into(),
                Some("k".into()),
            ),
        ));
        assert_eq!(p.name(), "openai");
    }
}
