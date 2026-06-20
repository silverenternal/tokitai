//! T-034: response cache.
//!
//! Keyed by a blake3 hash of `(model, system_prompt, messages,
//! tool_envelopes)`. Two storage backends are supported:
//!
//! - In-memory (default): a `HashMap<String, Vec<u8>>` that is lost
//!   on process exit. Best for CI runs and dev loops.
//! - SQLite (gated on the `sqlite-cache` feature): durable across
//!   process restarts. The schema is a single
//!   `cache(key BLOB PRIMARY KEY, value BLOB NOT NULL)` table.
//!
//! The cache is provider-agnostic: a single cache instance can hold
//! responses from any combination of OpenAI / Anthropic / Ollama.

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::provider::ChatMessage;
use tokitai_core::ToolDefinition;

/// One cached response. Persisted as a single JSON blob so the
/// `serde_json` round-trip is the only codec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    /// The model's plain-text content (may be empty).
    pub content: String,
    /// Tool calls the model wanted dispatched. Stored as
    /// `(id, name, arguments)` triples — the dispatcher can
    /// rebuild a `ProviderToolCall` from this directly.
    pub tool_calls: Vec<CachedToolCall>,
    /// Token-usage report (provider-dependent; may be `None`).
    pub usage: Option<CachedUsage>,
}

/// One cached tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedToolCall {
    /// Provider-assigned call id.
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// Parsed arguments object.
    pub arguments: Value,
}

/// Cached token-usage telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedUsage {
    /// Prompt tokens.
    pub prompt_tokens: u64,
    /// Completion tokens.
    pub completion_tokens: u64,
    /// Total tokens.
    pub total_tokens: u64,
}

impl From<&crate::provider::CompletionResponse> for CachedResponse {
    fn from(r: &crate::provider::CompletionResponse) -> Self {
        Self {
            content: r.content.clone(),
            tool_calls: r
                .tool_calls
                .iter()
                .map(|tc| CachedToolCall {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                })
                .collect(),
            usage: r.usage.as_ref().map(|u| CachedUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
        }
    }
}

impl From<&CachedResponse> for crate::provider::CompletionResponse {
    fn from(c: &CachedResponse) -> Self {
        Self {
            content: c.content.clone(),
            tool_calls: c
                .tool_calls
                .iter()
                .map(|tc| crate::provider::ProviderToolCall {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                })
                .collect(),
            usage: c.usage.as_ref().map(|u| crate::provider::UsageReport {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
        }
    }
}

/// Compute the cache key for a chat-completion request. The hash
/// is the blake3 of a stable JSON serialisation of every input
/// that affects the model's reply. Field order is fixed in
/// `CacheKey`.
pub fn cache_key(
    model: &str,
    system: Option<&str>,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> String {
    // Pre-render the tools as OpenAI envelopes because that's the
    // canonical shape (the OpenAI / Anthropic / Ollama envelopes
    // are all derived from the same `ToolDefinition` and would
    // hash to the same value when normalised).
    let tools_json: Vec<Value> = tools.iter().map(|t| t.to_openai_function()).collect();

    let key = CacheKey {
        model: model.to_string(),
        system: system.map(str::to_string),
        messages: messages.to_vec(),
        tools: tools_json,
    };
    // `CacheKey` only contains primitives, strings, `Vec<Value>`,
    // and `Vec<ChatMessage>` — every variant serialises by
    // contract. If serialisation ever does fail we degrade to an
    // empty key rather than panicking the whole LLM loop; a cache
    // miss is harmless, a panic in a CLI is not.
    let bytes = match serde_json::to_vec(&key) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("cache_key serialisation failed: {e}");
            return String::new();
        }
    };
    let mut hasher = Hasher::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    // Hex is friendlier in logs than raw bytes.
    digest.to_hex().to_string()
}

#[derive(Serialize, Deserialize)]
struct CacheKey {
    model: String,
    system: Option<String>,
    messages: Vec<ChatMessage>,
    tools: Vec<Value>,
}

/// In-memory cache backend. The default for the binary.
#[derive(Debug, Default)]
pub struct InMemoryCache {
    inner: std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

/// Acquire the cache mutex. Recovers from poisoning (a panic
/// while holding the lock) by returning the inner map after
/// logging; the alternative — silently returning `None` — would
/// mask the panic and leave stale entries invisible to readers.
fn lock_cache<K, V>(
    m: &std::sync::Mutex<std::collections::HashMap<K, V>>,
) -> std::sync::MutexGuard<'_, std::collections::HashMap<K, V>> {
    match m.lock() {
        Ok(g) => g,
        Err(p) => {
            tracing::warn!("cache mutex poisoned: {p}");
            p.into_inner()
        }
    }
}

impl InMemoryCache {
    /// Build an empty in-memory cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a cached response by key. Returns `None` on miss.
    pub fn get(&self, key: &str) -> Option<CachedResponse> {
        let guard = lock_cache(&self.inner);
        let bytes = guard.get(key)?;
        serde_json::from_slice(bytes).ok()
    }

    /// Insert a response under `key`, overwriting any prior entry.
    pub fn put(&self, key: String, value: &CachedResponse) {
        let Ok(bytes) = serde_json::to_vec(value) else {
            return;
        };
        let mut guard = lock_cache(&self.inner);
        guard.insert(key, bytes);
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        lock_cache(&self.inner).len()
    }

    /// True when the cache holds no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Trait alias: any backend that can `get` / `put` by string key.
/// Kept as a trait so `infer::run` accepts both the in-memory
/// backend and a future SQLite backend (gated on `sqlite-cache`).
pub trait CacheBackend: Send + Sync {
    /// Look up `key`. Returns `None` on miss.
    fn get(&self, key: &str) -> Option<CachedResponse>;
    /// Store `value` under `key`.
    fn put(&self, key: String, value: &CachedResponse);
}

impl CacheBackend for InMemoryCache {
    fn get(&self, key: &str) -> Option<CachedResponse> {
        self.get(key)
    }
    fn put(&self, key: String, value: &CachedResponse) {
        self.put(key, value);
    }
}

// ---------------------------------------------------------------------------
// T-047: Tool-result cache
// ---------------------------------------------------------------------------
//
// Self-consistency patterns (5 samples calling the same `sympy_solve`
// tool with identical args) hit the MCP tool backend 5 times for the
// same answer. This cache deduplicates that work by keying on
// `(tool_name, serialized arguments)` instead of the full
// `(model, messages, tools)` envelope. The TTL is intentionally short
// (60 s by default): tool results can change underneath us (e.g. a
// `current_time` tool, an external HTTP fetch), so a long TTL would
// silently stale-cache. Short TTLs make a cache miss on every new
// `infer` invocation likely; that's the correct trade-off for the
// self-consistency use case (multiple samples within one invocation).

/// Default tool-cache TTL when none is supplied. Short enough that
/// re-runs of an `infer` invocation do not silently stale-cache
/// results from a previous run.
pub const DEFAULT_TOOL_CACHE_TTL: Duration = Duration::from_secs(60);

/// Compute the blake3 hex digest that the `ToolCache` uses as its
/// storage key. The arguments are serialised to JSON in a canonical
/// field order (whatever `serde_json::to_string` produces for the
/// `Value`) — the cache contract is "same tool + same args → hit",
/// not bit-identical argument serialisation.
fn tool_cache_hash(tool: &str, args: &Value) -> String {
    // Serialise arguments first; fall back to the empty string on a
    // non-serialisable `Value` so a single bad call cannot poison the
    // cache for the rest of the process.
    let args_bytes = match serde_json::to_vec(args) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("tool_cache: argument serialisation failed: {e}");
            Vec::new()
        }
    };
    let mut hasher = Hasher::new();
    hasher.update(tool.as_bytes());
    hasher.update(&[0u8]);
    hasher.update(&args_bytes);
    hasher.finalize().to_hex().to_string()
}

/// One tool-cache entry: the serialised result together with the
/// instant it was inserted (used for TTL checks).
#[derive(Debug, Clone)]
struct ToolCacheEntry {
    inserted_at: Instant,
    value: Vec<u8>,
}

/// Tool-result cache. Wraps an `InMemoryCache`-like inner store
/// with a TTL and a `(tool_name, args)`-derived key.
///
/// Construct via [`ToolCache::new`] (default TTL) or
/// [`ToolCache::with_capacity_and_ttl`]. Read/write goes through
/// [`ToolCache::get_or_compute`].
#[derive(Debug)]
pub struct ToolCache {
    store: Arc<std::sync::Mutex<std::collections::HashMap<String, ToolCacheEntry>>>,
    ttl: Duration,
}

impl Default for ToolCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCache {
    /// Build a `ToolCache` with the default TTL (60 s) and an
    /// unbounded in-memory store.
    pub fn new() -> Self {
        Self::with_capacity_and_ttl(1024, DEFAULT_TOOL_CACHE_TTL)
    }

    /// Build a `ToolCache` with an explicit entry cap and TTL.
    /// `cap` is a soft hint for the initial `HashMap` capacity;
    /// entries are never evicted by size, only by TTL expiry.
    pub fn with_capacity_and_ttl(cap: usize, ttl: Duration) -> Self {
        Self {
            store: Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::with_capacity(cap),
            )),
            ttl,
        }
    }

    /// Look up a cached result for `(tool, args)`. Returns `Some`
    /// when a non-expired entry exists, `None` otherwise (including
    /// when the entry exists but is past its TTL).
    pub fn get(&self, tool: &str, args: &Value) -> Option<Value> {
        let key = tool_cache_hash(tool, args);
        let mut guard = lock_cache(&self.store);
        let entry = guard.get(&key)?;
        if entry.inserted_at.elapsed() >= self.ttl {
            // TTL expired — drop and treat as a miss.
            guard.remove(&key);
            return None;
        }
        serde_json::from_slice(&entry.value).ok()
    }

    /// Insert `value` under `(tool, args)`, replacing any prior
    /// entry. The timestamp is captured at call time and the TTL
    /// clock starts from that instant.
    pub fn put(&self, tool: &str, args: &Value, value: &Value) {
        let Ok(bytes) = serde_json::to_vec(value) else {
            tracing::warn!("tool_cache: failed to serialise result; skipping insert");
            return;
        };
        let key = tool_cache_hash(tool, args);
        let mut guard = lock_cache(&self.store);
        guard.insert(
            key,
            ToolCacheEntry {
                inserted_at: Instant::now(),
                value: bytes,
            },
        );
    }

    /// Drop every cached entry. Primarily useful for tests.
    pub fn clear(&self) {
        let mut guard = lock_cache(&self.store);
        guard.clear();
    }

    /// Number of entries currently in the cache (including any
    /// whose TTL has elapsed but have not yet been read).
    pub fn len(&self) -> usize {
        lock_cache(&self.store).len()
    }

    /// `true` when no entries are stored.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Memoised dispatch: if `(tool, args)` has a non-expired
    /// entry, return the cached `Value` without invoking `f`.
    /// Otherwise call `f`, store the result, and return it.
    ///
    /// This is the hot path for the self-consistency use case: the
    /// same `(tool, args)` pair can flow through here many times
    /// per `infer` invocation, and only the first call ever
    /// reaches the underlying tool provider.
    ///
    /// The error type is generic so a `ToolError` from a
    /// `ToolProvider::call_tool` closure flows through unchanged
    /// without an extra `.map_err`. Callers that already have an
    /// `anyhow::Error` can pass it directly.
    pub async fn get_or_compute<F, Fut, E>(
        &self,
        tool: &str,
        args: &Value,
        f: F,
    ) -> std::result::Result<Value, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = std::result::Result<Value, E>>,
    {
        if let Some(hit) = self.get(tool, args) {
            tracing::debug!(tool, "tool_cache hit");
            return Ok(hit);
        }
        tracing::debug!(tool, "tool_cache miss; dispatching");
        let value = f().await?;
        self.put(tool, args, &value);
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_deterministic_for_identical_input() {
        let tools = vec![ToolDefinition::new(
            "add",
            "add two numbers",
            r#"{"type":"object"}"#,
        )];
        let messages = vec![ChatMessage::User {
            content: "hi".into(),
        }];
        let k1 = cache_key("gpt-4o", None, &messages, &tools);
        let k2 = cache_key("gpt-4o", None, &messages, &tools);
        assert_eq!(k1, k2);
    }

    #[test]
    fn cache_key_changes_when_prompt_changes() {
        let tools = vec![ToolDefinition::new(
            "add",
            "add two numbers",
            r#"{"type":"object"}"#,
        )];
        let m1 = vec![ChatMessage::User {
            content: "hi".into(),
        }];
        let m2 = vec![ChatMessage::User {
            content: "hello".into(),
        }];
        let k1 = cache_key("gpt-4o", None, &m1, &tools);
        let k2 = cache_key("gpt-4o", None, &m2, &tools);
        assert_ne!(k1, k2);
    }

    #[test]
    fn in_memory_cache_round_trip() {
        let cache = InMemoryCache::new();
        let resp = CachedResponse {
            content: "42".into(),
            tool_calls: vec![],
            usage: None,
        };
        cache.put("k".into(), &resp);
        let got = cache.get("k").unwrap();
        assert_eq!(got.content, "42");
    }

    #[test]
    fn cache_key_differs_with_system() {
        let tools = vec![ToolDefinition::new(
            "add",
            "add two numbers",
            r#"{"type":"object"}"#,
        )];
        let messages = vec![ChatMessage::User {
            content: "hi".into(),
        }];
        let k_none = cache_key("gpt-4o", None, &messages, &tools);
        let k_some = cache_key("gpt-4o", Some("be concise"), &messages, &tools);
        assert_ne!(k_none, k_some);
    }

    #[test]
    fn cache_key_differs_with_tools() {
        let messages = vec![ChatMessage::User {
            content: "hi".into(),
        }];
        let t1 = vec![ToolDefinition::new("add", "add", r#"{"type":"object"}"#)];
        let t2 = vec![ToolDefinition::new("mul", "mul", r#"{"type":"object"}"#)];
        let k1 = cache_key("gpt-4o", None, &messages, &t1);
        let k2 = cache_key("gpt-4o", None, &messages, &t2);
        assert_ne!(k1, k2);
    }

    #[test]
    fn cache_round_trip_with_tool_calls() {
        let cache = InMemoryCache::new();
        let resp = CachedResponse {
            content: String::new(),
            tool_calls: vec![CachedToolCall {
                id: "call_1".into(),
                name: "echo".into(),
                arguments: serde_json::json!({"x": 1}),
            }],
            usage: None,
        };
        cache.put("k".into(), &resp);
        let got = cache.get("k").expect("cache hit");
        assert_eq!(got.tool_calls.len(), 1);
        assert_eq!(got.tool_calls[0].id, "call_1");
        assert_eq!(got.tool_calls[0].name, "echo");
        assert_eq!(got.tool_calls[0].arguments, serde_json::json!({"x": 1}));
    }

    #[test]
    fn cache_round_trip_with_usage() {
        let cache = InMemoryCache::new();
        let resp = CachedResponse {
            content: "ok".into(),
            tool_calls: vec![],
            usage: Some(CachedUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        };
        cache.put("k".into(), &resp);
        let got = cache.get("k").expect("cache hit");
        let usage = got.usage.expect("usage preserved");
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn cache_overwrite() {
        let cache = InMemoryCache::new();
        let resp1 = CachedResponse {
            content: "first".into(),
            tool_calls: vec![],
            usage: None,
        };
        let resp2 = CachedResponse {
            content: "second".into(),
            tool_calls: vec![],
            usage: None,
        };
        cache.put("k".into(), &resp1);
        cache.put("k".into(), &resp2);
        let got = cache.get("k").expect("cache hit");
        assert_eq!(got.content, "second");
    }

    #[test]
    fn cache_conversion_to_completion_response_preserves_fields() {
        let cr = CachedResponse {
            content: "hi".into(),
            tool_calls: vec![CachedToolCall {
                id: "c".into(),
                name: "echo".into(),
                arguments: serde_json::json!({}),
            }],
            usage: Some(CachedUsage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
            }),
        };
        let resp: crate::provider::CompletionResponse = (&cr).into();
        assert_eq!(resp.content, "hi");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "echo");
        let u = resp.usage.expect("usage");
        assert_eq!(u.total_tokens, 3);
    }

    #[test]
    fn cache_miss_returns_none() {
        let cache = InMemoryCache::new();
        assert!(cache.get("nope").is_none());
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    // -----------------------------------------------------------------
    // T-047: ToolCache
    // -----------------------------------------------------------------

    /// Five identical `(tool, args)` lookups should hit the cached
    /// value 4 times after the first dispatch. The closure is invoked
    /// exactly once — verified by an atomic counter.
    #[tokio::test]
    async fn tool_cache_five_identical_calls_yield_four_hits() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = ToolCache::with_capacity_and_ttl(8, Duration::from_secs(60));
        let calls = Arc::new(AtomicUsize::new(0));
        let args = serde_json::json!({"eq": "x^2-4", "var": "x"});

        for _ in 0..5 {
            let calls = Arc::clone(&calls);
            let value = cache
                .get_or_compute("sympy_solve", &args, || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, anyhow::Error>(serde_json::json!({"roots": [-2, 2]}))
                })
                .await
                .expect("get_or_compute");
            assert_eq!(value, serde_json::json!({"roots": [-2, 2]}));
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "exactly one dispatch despite 5 calls"
        );
        assert_eq!(cache.len(), 1);
    }

    /// Distinct arguments must produce distinct cache entries so
    /// they do not collide. Verifies the key includes the args.
    #[tokio::test]
    async fn tool_cache_distinct_args_get_distinct_entries() {
        let cache = ToolCache::with_capacity_and_ttl(8, Duration::from_secs(60));
        cache
            .get_or_compute(
                "sympy_solve",
                &serde_json::json!({"eq": "x^2-4"}),
                || async { Ok::<_, anyhow::Error>(serde_json::json!("eq1")) },
            )
            .await
            .unwrap();
        cache
            .get_or_compute(
                "sympy_solve",
                &serde_json::json!({"eq": "x^2-9"}),
                || async { Ok::<_, anyhow::Error>(serde_json::json!("eq2")) },
            )
            .await
            .unwrap();
        cache
            .get_or_compute(
                "other_tool",
                &serde_json::json!({"eq": "x^2-4"}),
                || async { Ok::<_, anyhow::Error>(serde_json::json!("other")) },
            )
            .await
            .unwrap();
        assert_eq!(cache.len(), 3, "three distinct (tool, args) entries");

        // Each entry must round-trip to its own value.
        assert_eq!(
            cache.get("sympy_solve", &serde_json::json!({"eq": "x^2-4"})),
            Some(serde_json::json!("eq1"))
        );
        assert_eq!(
            cache.get("sympy_solve", &serde_json::json!({"eq": "x^2-9"})),
            Some(serde_json::json!("eq2"))
        );
        assert_eq!(
            cache.get("other_tool", &serde_json::json!({"eq": "x^2-4"})),
            Some(serde_json::json!("other"))
        );
    }

    /// A TTL of zero should expire immediately — the first
    /// `get_or_compute` inserts, the second treats it as a miss.
    /// We assert the closure ran twice (with no sleeps, so this is
    /// reliable on any clock resolution).
    #[tokio::test]
    async fn tool_cache_ttl_expiry_re_dispatches() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = ToolCache::with_capacity_and_ttl(8, Duration::from_secs(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let args = serde_json::json!({"x": 1});
        for _ in 0..3 {
            let calls = Arc::clone(&calls);
            cache
                .get_or_compute("t", &args, || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, anyhow::Error>(serde_json::json!(null))
                })
                .await
                .unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    /// `ToolCache::get` (the bare lookup, without the closure
    /// path) must return `None` when the cache is empty and `Some`
    /// only for keys that have been put.
    #[tokio::test]
    async fn tool_cache_miss_returns_none_when_not_present() {
        let cache = ToolCache::new();
        assert!(cache.is_empty());
        assert!(cache.get("nope", &serde_json::json!({})).is_none());
        cache.put(
            "echo",
            &serde_json::json!({"x": 1}),
            &serde_json::json!("ok"),
        );
        assert_eq!(
            cache.get("echo", &serde_json::json!({"x": 1})),
            Some(serde_json::json!("ok"))
        );
        assert!(cache.get("echo", &serde_json::json!({"x": 2})).is_none());
    }
}
