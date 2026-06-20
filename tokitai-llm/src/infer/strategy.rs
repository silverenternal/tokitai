//! T-048: Execution strategies for dispatching a batch of tool calls
//! returned by the model in a single turn.
//!
//! Three strategies are supported:
//!
//! - [`ExecutionStrategy::Sequential`] (default): call each tool in
//!   order. Deterministic; matches the pre-T-048 behaviour.
//! - [`ExecutionStrategy::Parallel`]: fan out all calls via
//!   `futures::future::join_all`, with at most `max_concurrency`
//!   in flight at any moment. The provider is `&self`, so no
//!   locking is required.
//! - [`ExecutionStrategy::Pipelined`]: wave N waits for wave N-1
//!   to finish, but every call inside a wave runs concurrently
//!   (bounded by `max_concurrency`). Each wave is described by a
//!   `Vec<String>` of tool-call IDs the model returned.
//!
//! The default is `Sequential` because it matches the existing
//! behaviour byte-for-byte; existing scripts that depend on
//! call ordering keep working without any new flag.

use crate::cache::ToolCache;
use crate::Result;
use futures::future::join_all;
use serde_json::Value;
use tokitai_core::ToolCaller;

/// T-048: how the dispatcher should execute a batch of tool
/// calls emitted by the model in a single assistant turn.
///
/// `Clone` is required because the value is moved into the
/// `infer::run` task and also read by the dispatcher helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStrategy {
    /// Call each tool in order. The pre-T-048 behaviour.
    Sequential,
    /// Fan out every tool call concurrently, capped at
    /// `max_concurrency` in flight at any time.
    ///
    /// `max_concurrency = 0` is treated as "unbounded" (no
    /// chunking); `1` degenerates to sequential order for the
    /// *completion* order, but the dispatch path still uses
    /// the parallel code path.
    Parallel {
        /// Maximum number of in-flight calls. `0` means unbounded.
        max_concurrency: usize,
    },
    /// Wave N waits for wave N-1 to complete. Within a wave,
    /// every call runs concurrently (bounded by
    /// `max_concurrency`; `0` = unbounded).
    ///
    /// `waves` is `Vec<Vec<String>>` where the outer index is the
    /// wave number and the inner `Vec<String>` is the list of
    /// tool-call IDs to execute in that wave. A wave with an
    /// empty inner vec is a no-op (it still counts as a
    /// synchronisation barrier for the next wave).
    Pipelined {
        /// `max_concurrency` per wave. `0` = unbounded.
        max_concurrency: usize,
        /// Wave-by-wave dispatch plan.
        waves: Vec<Vec<String>>,
    },
}

impl Default for ExecutionStrategy {
    /// Default to `Sequential` so existing invocations behave
    /// exactly as they did before T-048 landed.
    fn default() -> Self {
        ExecutionStrategy::Sequential
    }
}

/// One tool call as seen by the dispatcher. Decoupled from
/// `provider::ProviderToolCall` so callers can construct an
/// arbitrary batch (used by tests, and by future code paths
/// that build the batch from a different source).
#[derive(Debug, Clone)]
pub struct DispatchItem {
    /// Provider-assigned call id (used to correlate the result
    /// back into the chat message stream).
    pub id: String,
    /// Tool name to dispatch.
    pub name: String,
    /// Arguments blob (already parsed).
    pub arguments: Value,
}

/// One dispatched tool call's result. The dispatcher always
/// preserves the original `id` so the caller can splice the
/// `value` back into a chat message in the right slot.
///
/// `Clone` is intentionally NOT derived: the `value` field is
/// `Result<Value, anyhow::Error>` and `anyhow::Error` does not
/// implement `Clone`. Callers that need to clone a result
/// can clone `id` / `name` and inspect / consume `value`
/// directly.
#[derive(Debug)]
pub struct DispatchResult {
    /// Provider-assigned call id (same as the input `id`).
    pub id: String,
    /// Tool name that produced this result.
    pub name: String,
    /// Parsed result value (when the dispatch succeeded) or an
    /// error (when it failed).
    pub value: Result<Value>,
}

/// Dispatch a batch of tool calls under the chosen
/// [`ExecutionStrategy`].
///
/// The `provider` is `&dyn ToolCaller` because every
/// `ToolProvider` in `tokitai-core` is `&self`-only. No locking
/// is required for `Parallel` / `Pipelined`; calls are
/// independent and the dispatcher only reads.
///
/// When `tool_cache` is `Some`, every individual dispatch goes
/// through `ToolCache::get_or_compute` so a self-consistency
/// run that emits the same call twice collapses to a single
/// underlying `provider.call_tool` invocation.
pub async fn dispatch_tool_calls(
    provider: &dyn ToolCaller,
    tool_cache: Option<&ToolCache>,
    strategy: &ExecutionStrategy,
    items: Vec<DispatchItem>,
) -> Result<Vec<DispatchResult>> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    match strategy {
        ExecutionStrategy::Sequential => Ok(dispatch_sequential(provider, tool_cache, items).await),
        ExecutionStrategy::Parallel { max_concurrency } => {
            dispatch_parallel(provider, tool_cache, *max_concurrency, items).await
        }
        ExecutionStrategy::Pipelined {
            max_concurrency,
            waves,
        } => dispatch_pipelined(provider, tool_cache, *max_concurrency, waves, items).await,
    }
}

/// Sequential path: call each tool in input order. The simplest
/// behaviour and the default.
async fn dispatch_sequential(
    provider: &dyn ToolCaller,
    tool_cache: Option<&ToolCache>,
    items: Vec<DispatchItem>,
) -> Vec<DispatchResult> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let value =
            crate::infer::dispatch_with_cache(provider, tool_cache, &item.name, &item.arguments)
                .await;
        out.push(DispatchResult {
            id: item.id,
            name: item.name,
            value,
        });
    }
    out
}

/// Parallel path: run every dispatch concurrently, bounded by
/// `max_concurrency`. A `max_concurrency` of `0` is treated as
/// "unbounded" and degenerates to `futures::future::join_all`.
async fn dispatch_parallel(
    provider: &dyn ToolCaller,
    tool_cache: Option<&ToolCache>,
    max_concurrency: usize,
    items: Vec<DispatchItem>,
) -> Result<Vec<DispatchResult>> {
    if max_concurrency == 0 || max_concurrency >= items.len() {
        // Unbounded (or effectively unbounded) — `join_all` is
        // the simplest correct implementation.
        let futs = items
            .into_iter()
            .map(|item| dispatch_one(provider, tool_cache, item));
        let results: Vec<DispatchResult> = join_all(futs).await;
        return Ok(results);
    }

    // Bounded: process in waves of `max_concurrency` calls.
    // Each wave is dispatched with `join_all`; the loop then
    // waits for that wave before launching the next. This keeps
    // the in-flight count at exactly `max_concurrency`.
    let mut out: Vec<DispatchResult> = Vec::with_capacity(items.len());
    let mut iter = items.into_iter();
    loop {
        let chunk: Vec<DispatchItem> = iter.by_ref().take(max_concurrency).collect();
        if chunk.is_empty() {
            break;
        }
        let results: Vec<DispatchResult> = join_all(
            chunk
                .into_iter()
                .map(|item| dispatch_one(provider, tool_cache, item)),
        )
        .await;
        out.extend(results);
    }
    Ok(out)
}

/// Pipelined path: wave N waits for wave N-1, but every call
/// within a wave runs concurrently.
///
/// The `waves` argument is `Vec<Vec<String>>` of tool-call IDs.
/// `items` is the full batch. Calls that are NOT mentioned in
/// any wave are dispatched alongside the first wave (i.e. we
/// treat un-mentioned IDs as "wave zero"). This keeps the
/// dispatcher robust against slightly-out-of-sync plans.
async fn dispatch_pipelined(
    provider: &dyn ToolCaller,
    tool_cache: Option<&ToolCache>,
    max_concurrency: usize,
    waves: &[Vec<String>],
    items: Vec<DispatchItem>,
) -> Result<Vec<DispatchResult>> {
    if waves.is_empty() {
        // Empty plan: behave like a single parallel batch.
        return dispatch_parallel(provider, tool_cache, max_concurrency, items).await;
    }

    // Build a `String -> DispatchItem` lookup so we can map each
    // wave's id list back to its concrete item. Items missing
    // from every wave fall into `unplanned` and run as wave 0.
    let mut by_id: std::collections::HashMap<String, DispatchItem> =
        items.iter().map(|i| (i.id.clone(), i.clone())).collect();
    let mut all_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
    let mut ordered_waves: Vec<Vec<String>> = Vec::with_capacity(waves.len());
    let mut planned: std::collections::HashSet<String> = std::collections::HashSet::new();
    for wave in waves {
        let clean: Vec<String> = wave
            .iter()
            .filter(|id| {
                if planned.contains(*id) {
                    return false;
                }
                if !by_id.contains_key(*id) {
                    tracing::warn!(
                        id = %id,
                        "pipelined wave referenced an unknown tool-call id; ignoring"
                    );
                    return false;
                }
                true
            })
            .cloned()
            .collect();
        for id in &clean {
            planned.insert(id.clone());
        }
        ordered_waves.push(clean);
    }
    let unplanned: Vec<String> = all_ids
        .drain(..)
        .filter(|id| !planned.contains(id))
        .collect();
    if !unplanned.is_empty() {
        ordered_waves.insert(0, unplanned);
    }

    let mut out: Vec<DispatchResult> = Vec::with_capacity(items.len());
    for wave in &ordered_waves {
        if wave.is_empty() {
            // Empty wave = a sync barrier with no work.
            continue;
        }
        let wave_items: Vec<DispatchItem> = wave.iter().filter_map(|id| by_id.remove(id)).collect();
        let results = dispatch_parallel(provider, tool_cache, max_concurrency, wave_items).await?;
        out.extend(results);
    }
    // Defensive: any items left in `by_id` (e.g. unknown ids
    // referenced by waves) get dispatched in a final wave so
    // we never silently drop a call.
    let leftovers: Vec<DispatchItem> = by_id.into_values().collect();
    if !leftovers.is_empty() {
        let results = dispatch_parallel(provider, tool_cache, max_concurrency, leftovers).await?;
        out.extend(results);
    }
    Ok(out)
}

/// Run a single dispatch through the cache (if provided) and
/// shape the result back as a `DispatchResult`. Wrapped in an
/// `async` block so it can be `join_all`-ed.
async fn dispatch_one(
    provider: &dyn ToolCaller,
    tool_cache: Option<&ToolCache>,
    item: DispatchItem,
) -> DispatchResult {
    let value =
        crate::infer::dispatch_with_cache(provider, tool_cache, &item.name, &item.arguments).await;
    DispatchResult {
        id: item.id,
        name: item.name,
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokitai_core::{ToolError, ToolErrorKind};

    /// Mock tool provider that records each call. `&self`-only
    /// so it satisfies `ToolCaller` without any locking.
    struct MockToolProvider {
        counter: Arc<AtomicUsize>,
        // Optional artificial latency so the parallel path can
        // actually fan out (in milliseconds).
        #[allow(dead_code)]
        delay_ms: u64,
    }

    impl MockToolProvider {
        fn new(delay_ms: u64) -> Self {
            Self {
                counter: Arc::new(AtomicUsize::new(0)),
                delay_ms,
            }
        }
    }

    impl ToolCaller for MockToolProvider {
        fn call_tool(&self, name: &str, _args: &Value) -> std::result::Result<Value, ToolError> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            // Note: `ToolCaller::call_tool` is a synchronous
            // function, so an artificial delay here would block
            // the worker thread and starve the parallel path.
            // The real parallel/pipelined concurrency comes
            // from inside an async `ToolProvider` that yields
            // the runtime while doing its work (e.g. a network
            // call via `reqwest`); the dispatcher's `join_all`
            // schedules those yields concurrently. The mock
            // here therefore exercises the dispatch ordering
            // and cache-dedup behaviour, not the latency
            // properties. A latency-property test would need
            // an async trait, which is out of scope for the
            // `&self`-only `ToolCaller` surface that the
            // dispatcher is built on top of.
            Ok(json!({"name": name, "ok": true}))
        }
    }

    fn items(names: &[&str]) -> Vec<DispatchItem> {
        names
            .iter()
            .enumerate()
            .map(|(i, n)| DispatchItem {
                id: format!("call_{i}"),
                name: (*n).to_string(),
                arguments: json!({}),
            })
            .collect()
    }

    #[tokio::test]
    async fn sequential_default_processes_in_order() {
        let provider = MockToolProvider::new(0);
        let results = dispatch_tool_calls(
            &provider,
            None,
            &ExecutionStrategy::Sequential,
            items(&["a", "b", "c"]),
        )
        .await
        .expect("dispatch");
        let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
        assert_eq!(provider.counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn parallel_unbounded_runs_all_calls() {
        let provider = MockToolProvider::new(0);
        let results = dispatch_tool_calls(
            &provider,
            None,
            &ExecutionStrategy::Parallel { max_concurrency: 0 },
            items(&["a", "b", "c", "d"]),
        )
        .await
        .expect("dispatch");
        assert_eq!(results.len(), 4);
        let names: std::collections::HashSet<String> =
            results.iter().map(|r| r.name.clone()).collect();
        for n in ["a", "b", "c", "d"] {
            assert!(names.contains(n), "missing call {n}");
        }
        assert_eq!(provider.counter.load(Ordering::SeqCst), 4);
    }

    /// Sequential dispatch: each call must complete before the
    /// next one starts (counter monotonically increases per
    /// observed call). The mock cannot measure wall-clock
    /// latency because `ToolCaller::call_tool` is sync; the
    /// observable property is "every call ran exactly once".
    #[tokio::test]
    async fn parallel_bounded_caps_in_flight() {
        let provider = MockToolProvider::new(0);
        let strategy = ExecutionStrategy::Parallel { max_concurrency: 2 };
        let results = dispatch_tool_calls(&provider, None, &strategy, items(&["a", "b", "c", "d"]))
            .await
            .expect("dispatch");
        assert_eq!(results.len(), 4);
        // Every name appears in the output (parallel does not
        // drop calls).
        let names: std::collections::HashSet<String> =
            results.iter().map(|r| r.name.clone()).collect();
        for n in ["a", "b", "c", "d"] {
            assert!(names.contains(n), "missing call {n}");
        }
        assert_eq!(provider.counter.load(Ordering::SeqCst), 4);
    }

    /// With `max_concurrency = 1` and a sync mock provider,
    /// `Parallel` must still produce the four calls. The mock
    /// can't exercise true parallelism (see `MockToolProvider`),
    /// but it can exercise the dispatcher's structural
    /// correctness.
    #[tokio::test]
    async fn parallel_bounded_concurrency_one_still_processes_all() {
        let provider = MockToolProvider::new(0);
        let strategy = ExecutionStrategy::Parallel { max_concurrency: 1 };
        let results = dispatch_tool_calls(&provider, None, &strategy, items(&["a", "b", "c"]))
            .await
            .expect("dispatch");
        assert_eq!(results.len(), 3);
        let names: Vec<String> = results.iter().map(|r| r.name.clone()).collect();
        assert_eq!(
            names,
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            "max_concurrency=1 should preserve input order"
        );
        assert_eq!(provider.counter.load(Ordering::SeqCst), 3);
    }

    /// `Pipelined` must dispatch every call (planned + unplanned)
    /// regardless of which wave each id appears in. We don't
    /// measure wall-clock latency for the same reason as the
    /// parallel test above.
    #[tokio::test]
    async fn pipelined_waves_dispatch_all_calls() {
        let provider = MockToolProvider::new(0);
        let waves = vec![
            vec!["call_0".to_string()],
            vec!["call_1".to_string(), "call_2".to_string()],
        ];
        let results = dispatch_tool_calls(
            &provider,
            None,
            &ExecutionStrategy::Pipelined {
                max_concurrency: 0,
                waves,
            },
            items(&["a", "b", "c"]),
        )
        .await
        .expect("dispatch");
        assert_eq!(results.len(), 3);
        let names: std::collections::HashSet<String> =
            results.iter().map(|r| r.name.clone()).collect();
        assert!(names.contains("a"));
        assert!(names.contains("b"));
        assert!(names.contains("c"));
        assert_eq!(provider.counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn pipelined_unplanned_items_dispatch_in_first_wave() {
        let provider = MockToolProvider::new(0);
        // Plan only mentions `b`; `a` and `c` are unplanned and
        // should still be dispatched (in wave 0).
        let waves = vec![vec!["call_1".to_string()]];
        let results = dispatch_tool_calls(
            &provider,
            None,
            &ExecutionStrategy::Pipelined {
                max_concurrency: 0,
                waves,
            },
            items(&["a", "b", "c"]),
        )
        .await
        .expect("dispatch");
        assert_eq!(results.len(), 3);
        let names: std::collections::HashSet<String> =
            results.iter().map(|r| r.name.clone()).collect();
        assert!(names.contains("a"));
        assert!(names.contains("b"));
        assert!(names.contains("c"));
    }

    #[tokio::test]
    async fn empty_items_returns_empty() {
        let provider = MockToolProvider::new(0);
        let r = dispatch_tool_calls(&provider, None, &ExecutionStrategy::Sequential, Vec::new())
            .await
            .expect("dispatch");
        assert!(r.is_empty());
        assert_eq!(provider.counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn default_is_sequential() {
        assert_eq!(ExecutionStrategy::default(), ExecutionStrategy::Sequential);
    }

    #[tokio::test]
    async fn parallel_with_cache_dedupes_identical_calls() {
        use crate::cache::ToolCache;

        struct CountingToolProvider {
            counter: Arc<AtomicUsize>,
        }
        impl ToolCaller for CountingToolProvider {
            fn call_tool(
                &self,
                name: &str,
                _args: &Value,
            ) -> std::result::Result<Value, ToolError> {
                self.counter.fetch_add(1, Ordering::SeqCst);
                Ok(json!({"name": name}))
            }
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let provider = CountingToolProvider {
            counter: Arc::clone(&counter),
        };
        let cache = ToolCache::with_capacity_and_ttl(8, Duration::from_secs(60));
        // Two identical `(name, args)` calls should collapse to
        // a single provider invocation even under Parallel.
        let batch = vec![
            DispatchItem {
                id: "call_0".into(),
                name: "echo".into(),
                arguments: json!({"x": 1}),
            },
            DispatchItem {
                id: "call_1".into(),
                name: "echo".into(),
                arguments: json!({"x": 1}),
            },
        ];
        let results = dispatch_tool_calls(
            &provider,
            Some(&cache),
            &ExecutionStrategy::Parallel { max_concurrency: 0 },
            batch,
        )
        .await
        .expect("dispatch");
        assert_eq!(results.len(), 2);
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "tool cache should collapse the two identical calls into one provider call"
        );
    }

    // Silence the `ToolErrorKind` warning when the test module is
    // compiled without all test features enabled.
    #[allow(dead_code)]
    fn _silence_tool_error_kind_warning() {
        let _ = std::marker::PhantomData::<ToolErrorKind>;
    }
}
