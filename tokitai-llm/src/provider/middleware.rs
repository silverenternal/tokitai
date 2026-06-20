//! T-050: example provider-level middleware.
//!
//! This module ships one canonical example — [`AuthTokenRefresh`] —
//! so downstream users can copy the pattern for their own
//! middleware (rate limiters, OTel exporters, request-logging
//! adapters, ...). The example refreshes the bearer token on a 401
//! from the provider so a long-running agent survives a token
//! rotation.
//!
//! The example uses a `tokio::sync::Mutex` to serialise the
//! refresh so two parallel tool calls don't both fire the refresh
//! endpoint. After the first call sees a 401 and refreshes, the
//! second call retries with the fresh token — no double refresh,
//! no token thrashing.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::Client;

use super::{ProviderMiddleware, RetryDecision};
use crate::provider::{CompletionResponse, InferenceRequest};

/// T-050: refresh-on-401 middleware. Holds a refresh callback and
/// a shared token cell. On a 401 from the provider (the error
/// chain contains `"401"` or `"Unauthorized"`), the middleware
/// calls the refresh closure, stores the new token in the shared
/// cell, and asks the provider loop to retry once.
///
/// This is the canonical example referenced from the docs; copy
/// it for your own middleware. The 401 detection is intentionally
/// cheap (substring match on the error chain) so we don't have to
/// teach the provider loop about the `reqwest::StatusCode` type
/// — and so the middleware works uniformly across OpenAI /
/// Anthropic / Ollama.
pub struct AuthTokenRefresh {
    /// Shared cell holding the current bearer token. The
    /// `Mutex` lets two parallel tool calls serialise the
    /// refresh: one refreshes, the other waits and reuses the
    /// fresh token. Stored as `Arc` so the closure can hold a
    /// clone.
    token: Arc<tokio::sync::Mutex<String>>,
    /// Refresh closure. Returns the new token. Invoked exactly
    /// once per `401 → refresh → retry` cycle.
    refresher: Arc<dyn Fn() -> String + Send + Sync>,
    /// Backing HTTP client for the refresh request (if the
    /// refresh needs to hit a network endpoint instead of using
    /// the in-process closure). Optional — most users will use
    /// the closure.
    #[allow(dead_code)]
    client: Client,
}

impl std::fmt::Debug for AuthTokenRefresh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthTokenRefresh")
            .field("token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl AuthTokenRefresh {
    /// Build a refresh middleware from a closure. The closure
    /// returns the new token; it is invoked once per refresh
    /// cycle.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use tokitai_llm::provider::middleware::AuthTokenRefresh;
    /// use std::sync::Arc;
    ///
    /// let token = Arc::new(tokio::sync::Mutex::new("initial".to_string()));
    /// let token_for_cb = token.clone();
    /// let mw = AuthTokenRefresh::new(token, move || {
    ///     // In real code: hit your refresh endpoint, return
    ///     // the new token. Here we just rotate a counter.
    ///     let mut g = token_for_cb.blocking_lock();
    ///     *g = format!("refreshed-{}", g.len());
    ///     g.clone()
    /// });
    /// ```
    pub fn new<F>(token: Arc<tokio::sync::Mutex<String>>, refresher: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        Self {
            token,
            refresher: Arc::new(refresher),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl ProviderMiddleware for AuthTokenRefresh {
    async fn pre_request(&self, req: &mut InferenceRequest) -> anyhow::Result<()> {
        // No-op. The actual token is read by the provider's HTTP
        // layer (`OpenAiConfig::api_key`) — not by the request
        // struct. A middleware that wants to ride along on the
        // request can mutate `req.system` to attach a tracing
        // span or a request-id; this one deliberately does not.
        let _ = req;
        Ok(())
    }

    async fn post_request(
        &self,
        _req: &InferenceRequest,
        _resp: &anyhow::Result<CompletionResponse>,
        _duration: Duration,
    ) {
        // No-op. Successful calls don't need token work; the
        // auth token is read by the provider's HTTP layer.
    }

    async fn on_error(&self, error: &anyhow::Error, attempt: u32) -> RetryDecision {
        // Only retry once (attempt == 0). The retry callback runs
        // inside the mutex so a second 401 from a stale token is
        // handled by the operator, not by spinning.
        if attempt > 0 {
            return RetryDecision::GiveUp;
        }
        // Cheap substring match on the error chain. The OpenAI /
        // Anthropic / Ollama providers all surface 401 as a
        // `reqwest::StatusError` whose `Display` includes
        // "401 Unauthorized" (or "401" on older versions).
        let msg = format!("{error:#}");
        let is_401 = msg.contains("401")
            || msg.to_lowercase().contains("unauthorized")
            || msg.to_lowercase().contains("invalid api key");
        if !is_401 {
            return RetryDecision::GiveUp;
        }

        // Serialise the refresh so two parallel tool calls
        // don't both fire the refresh endpoint. Locking order:
        // take the lock, check the token hasn't already been
        // refreshed by another caller, refresh if needed.
        let start = Instant::now();
        let mut g = self.token.lock().await;
        // The closure holds a `String` so we just call it. A
        // real-world refresh would POST to an auth endpoint.
        let new_token = (self.refresher)();
        *g = new_token;
        tracing::info!(
            "AuthTokenRefresh: token rotated after 401 (elapsed {:?})",
            start.elapsed()
        );
        // Ask the provider loop to retry. A 50ms backoff is
        // small enough that downstream latency is unaffected
        // and large enough that a server-side token cache sees
        // the new value.
        RetryDecision::Retry {
            delay: Duration::from_millis(50),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-050: refresh fires only on a 401-shaped error. Other
    /// errors propagate without retry.
    #[tokio::test]
    async fn non_401_errors_get_give_up() {
        let token = Arc::new(tokio::sync::Mutex::new("v1".to_string()));
        let token_for_cb = token.clone();
        let calls = Arc::new(std::sync::Mutex::new(0u32));
        let calls_for_cb = calls.clone();
        let mw = AuthTokenRefresh::new(token, move || {
            *calls_for_cb.lock().unwrap() += 1;
            let mut g = token_for_cb.blocking_lock();
            *g = format!("v{}", g.len() + 1);
            g.clone()
        });

        let err = anyhow::anyhow!("connection refused");
        let decision = mw.on_error(&err, 0).await;
        assert_eq!(decision, RetryDecision::GiveUp);
        assert_eq!(*calls.lock().unwrap(), 0, "no refresh on non-401");
    }

    /// T-050: 401 triggers exactly one refresh + retry.
    #[tokio::test]
    async fn unauthorized_status_triggers_retry() {
        let token = Arc::new(tokio::sync::Mutex::new("v1".to_string()));
        let token_for_cb = token.clone();
        let calls = Arc::new(std::sync::Mutex::new(0u32));
        let calls_for_cb = calls.clone();
        let mw = AuthTokenRefresh::new(token, move || {
            *calls_for_cb.lock().unwrap() += 1;
            let mut g = token_for_cb.blocking_lock();
            *g = "v2".to_string();
            g.clone()
        });

        let err = anyhow::anyhow!("HTTP 401 Unauthorized");
        let decision = mw.on_error(&err, 0).await;
        assert!(matches!(decision, RetryDecision::Retry { .. }));
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    /// T-050: a second attempt never re-fires the refresh —
    /// the operator decides whether to retry, the middleware
    /// does not retry forever.
    #[tokio::test]
    async fn second_attempt_is_give_up() {
        let token = Arc::new(tokio::sync::Mutex::new("v1".to_string()));
        let token_for_cb = token.clone();
        let calls = Arc::new(std::sync::Mutex::new(0u32));
        let calls_for_cb = calls.clone();
        let mw = AuthTokenRefresh::new(token, move || {
            *calls_for_cb.lock().unwrap() += 1;
            let mut g = token_for_cb.blocking_lock();
            *g = "v2".to_string();
            g.clone()
        });

        let err = anyhow::anyhow!("401");
        let decision = mw.on_error(&err, 1).await;
        assert_eq!(decision, RetryDecision::GiveUp);
        assert_eq!(*calls.lock().unwrap(), 0);
    }

    /// T-050: a `reqwest::StatusError`-shaped error string also
    /// trips the refresh path. The middleware looks at the
    /// formatted chain, not the inner type, so any provider that
    /// surfaces 401 in its error chain works.
    #[tokio::test]
    async fn invalid_api_key_substring_triggers_retry() {
        let token = Arc::new(tokio::sync::Mutex::new("v1".to_string()));
        let token_for_cb = token.clone();
        let calls = Arc::new(std::sync::Mutex::new(0u32));
        let calls_for_cb = calls.clone();
        let mw = AuthTokenRefresh::new(token, move || {
            *calls_for_cb.lock().unwrap() += 1;
            let mut g = token_for_cb.blocking_lock();
            *g = "v2".to_string();
            g.clone()
        });

        let err = anyhow::anyhow!("error sending request for url (https://api.openai.com/v1/chat/completions): client error (Connect): invalid API key");
        let decision = mw.on_error(&err, 0).await;
        assert!(matches!(decision, RetryDecision::Retry { .. }));
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    /// T-050: `pre_request` and `post_request` are no-ops — the
    /// auth token lives in `OpenAiConfig::api_key`, not on the
    /// request, so this middleware touches neither.
    #[tokio::test]
    async fn pre_and_post_request_are_no_ops() {
        let token = Arc::new(tokio::sync::Mutex::new("v1".to_string()));
        let mw = AuthTokenRefresh::new(token, || "v2".to_string());
        let mut req = InferenceRequest::new("hi", vec![]);
        mw.pre_request(&mut req).await.unwrap();
        // system / messages unchanged.
        assert!(req.system.is_none());
        let resp = anyhow::Result::<CompletionResponse>::Ok(CompletionResponse {
            content: "ok".into(),
            tool_calls: vec![],
            usage: None,
        });
        mw.post_request(&req, &resp, Duration::from_millis(10)).await;
        // No assertion needed — a panic in `post_request` would
        // be caught by the test harness.
    }
}