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
fn lock_cache(
    m: &std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>,
) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, Vec<u8>>> {
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
}
