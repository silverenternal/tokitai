//! Example: combining `#[tool]` with the resilience decorators
//! `#[retry]`, `#[rate_limit]`, and `#[circuit_breaker]` to wrap
//! AI-callable methods with production-grade fault-tolerance at
//! compile time.
//!
//! Each decorator is applied as a normal proc-macro attribute on a
//! `#[tool]` method. The decorator re-writes the function body
//! (preserving the signature), so the call site the AI sees via
//! `call_tool` is already wrapped.
//!
//! Note: this example imports `tokitai_macros::*` directly. In a
//! real project the resilience macros would be re-exported by
//! `tokitai` alongside `tool`; here we hit them through the
//! proc-macro crate for clarity.
//!
//! Run with:
//! ```text
//! cargo run --example resilient_tool
//! ```
//! (Requires adding `[[example]] name = "resilient_tool"` to
//! `examples/Cargo.toml`.)

use tokitai::{tool, ToolProvider};
use tokitai_macros::{circuit_breaker, rate_limit, retry};

/// A small service whose methods demonstrate the three
/// resilience decorators.
#[derive(Default)]
pub struct ResilientService {
    /// Counter shared with the test below to prove the retry
    /// decorator actually invokes the body more than once.
    pub call_count: std::sync::atomic::AtomicU32,
}

#[tool]
impl ResilientService {
    /// Fetch a URL with at most 3 attempts, exponential backoff,
    /// and a small random jitter between attempts.
    #[retry(max = 3, backoff = "exponential", jitter = true)]
    pub async fn fetch_url(&self, url: String) -> Result<String, String> {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if url.is_empty() {
            return Err("empty url".to_string());
        }
        Ok(format!("fetched: {}", url))
    }

    /// Emit a log line, throttled to 10 messages per second with
    /// a 20-message burst capacity.
    #[rate_limit(rps = 10, burst = 20)]
    pub fn log_event(&self, message: String) -> String {
        format!("logged: {}", message)
    }

    /// Call an external service behind a circuit breaker that
    /// opens after 5 consecutive failures and re-tries probing
    /// after 30 seconds.
    #[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]
    pub async fn call_external(&self, endpoint: String) -> Result<String, String> {
        if endpoint.is_empty() {
            return Err("missing endpoint".to_string());
        }
        Ok(format!("called {}", endpoint))
    }
}

fn main() {
    let svc = ResilientService::default();

    // Print the compile-time tool definitions.
    let defs = svc.tool_definitions();
    println!("Generated {} tool definition(s):", defs.len());
    for d in defs {
        println!("  - {}", d.name);
    }

    // Exercise the retry decorator at runtime: every call fails
    // and the call count should climb to 3.
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let r = svc.fetch_url(String::new()).await;
        println!("fetch_url(empty) -> {:?} after {} attempts",
                 r,
                 svc.call_count.load(std::sync::atomic::Ordering::SeqCst));

        let r = svc.call_external("/health".to_string()).await;
        println!("call_external -> {:?}", r);
    });
}
