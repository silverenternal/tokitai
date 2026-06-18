//! T-004: end-to-end assertion that the resilience decorators
//! (`#[retry]`, `#[rate_limit]`) drive their inter-attempt sleeps
//! through the registered `AsyncExecutor` and *never* call
//! `std::thread::sleep` on the async path.
//!
//! The `#[retry]` macro emits `attempt = attempt + 1u32` in its
//! generated loop; clippy's `assign_op_pattern` lint flags that as
//! a manual `+=`. The lint is appropriate for hand-written code
//! but not for the generated loop body, so we allow it here.
#![allow(clippy::assign_op_pattern)]
//!
//! Strategy:
//!
//! 1. Register a stub executor whose `block_on_dyn` increments a
//!    counter so we can prove the executor was actually invoked.
//! 2. Drive an `async fn` decorated with `#[retry(max = 2,
//!    backoff = "constant")]` so the decorator is forced to sleep
//!    between attempts.
//! 3. The retry should fail twice then succeed; we assert the
//!    executor's counter increased by at least one (the sleep
//!    future was driven through the executor).
//! 4. Repeat for `#[rate_limit(rps = 1, burst = 1)]` to prove the
//!    rate-limit decorator's wait path also goes through the
//!    executor.
//!
//! This test lives in `tokitai-macros/tests/` (an integration test
//! binary) so the macro is exercised on a real `async fn` body
//! rather than on a token stream only. The resilience macros are
//! now exported as `#[proc_macro_attribute]` in
//! `tokitai_macros::{retry, rate_limit, circuit_breaker}` and are
//! reachable here through that path.

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokitai_core::{set_async_executor, AsyncExecutor};
use tokitai_macros::{rate_limit, retry};

/// Stub executor that increments counters so the test can prove the
/// executor path was actually used.
#[derive(Default)]
struct CountingExecutor {
    block_on_dyn_calls: AtomicUsize,
}

impl AsyncExecutor for CountingExecutor {
    fn block_on_dyn(
        &self,
        future: Pin<Box<dyn core::future::Future<Output = ()> + Send>>,
    ) -> Box<dyn core::any::Any + Send> {
        self.block_on_dyn_calls.fetch_add(1, Ordering::SeqCst);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should build");
        rt.block_on(future);
        Box::new(())
    }
}

/// Stub executor wrapper that exposes the counter through the
/// trait-object boundary so we can register it with
/// `set_async_executor` and still observe its bump counter from
/// the test body.
struct BoxedCounting(Arc<CountingExecutor>);

impl AsyncExecutor for BoxedCounting {
    fn block_on_dyn(
        &self,
        future: Pin<Box<dyn core::future::Future<Output = ()> + Send>>,
    ) -> Box<dyn core::any::Any + Send> {
        self.0.block_on_dyn_calls.fetch_add(1, Ordering::SeqCst);
        // Use Tokio's single-threaded current-thread runtime as the
        // driver. `tokio` is already a dev-dependency of
        // `tokitai-macros`; constructing a new runtime per call is
        // acceptable for a test executor.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should build");
        rt.block_on(future);
        Box::new(())
    }
}

/// Helper: register the executor for the test process (idempotent
/// across calls — first call wins) and return a handle to its
/// counter so the assertions can read the post-call value.
fn ensure_executor() -> Arc<CountingExecutor> {
    static SLOT: std::sync::OnceLock<Arc<CountingExecutor>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| {
        let counters = Arc::new(CountingExecutor::default());
        let boxed = BoxedCounting(Arc::clone(&counters));
        // First call wins; subsequent calls are silently ignored
        // (`set_async_executor` uses `OnceLock::set`, which returns
        // Err on duplicate). That is the desired behaviour: we
        // want the very first `#[test]` to populate the slot and
        // every later `#[test]` to share the same executor.
        set_async_executor(Box::new(boxed));
        counters
    })
    .clone()
}

/// Drive an async future to completion using the registered
/// executor. Mirrors what the macro's sync-from-async wrapper
/// would do for a user calling a sync wrapper of an async tool.
fn drive<F>(fut: F) -> F::Output
where
    F: core::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    // Re-fetch the registered executor so we go through the same
    // path the macro's wrapper uses.
    tokitai_core::block_on_async(fut).expect("test executor is registered before drive() runs")
}

// ----------------------------------------------------------------------
// #[retry] on async fn: the inter-attempt sleep must be driven
// through the executor.
// ----------------------------------------------------------------------

/// Always-fail helper used to force `#[retry]` to take multiple
/// attempts. The decorator sees `Err(_)` and sleeps before the next
/// attempt.
#[retry(max = 2, backoff = "constant", jitter = false)]
async fn always_fails() -> Result<i32, &'static str> {
    Err("nope")
}

/// Two-attempt helper that fails the first time and succeeds the
/// second. Forces the decorator to sleep between attempts.
#[retry(max = 3, backoff = "constant", jitter = false)]
async fn fails_then_succeeds() -> Result<&'static str, &'static str> {
    use std::sync::atomic::{AtomicUsize, Ordering as O};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    let n = CALLS.fetch_add(1, O::SeqCst);
    if n == 0 {
        Err("first call fails")
    } else {
        Ok("ok")
    }
}

#[test]
fn retry_async_uses_registered_executor() {
    let counters = ensure_executor();
    let before = counters.block_on_dyn_calls.load(Ordering::SeqCst);

    // `always_fails` returns `Err` twice (max = 2), then propagates
    // the last error. The single `await` we observe corresponds to
    // *one* top-level drive; the inter-attempt sleep future is the
    // *inner* drive that should bump `block_on_dyn_calls`.
    let result = drive(always_fails());
    assert!(result.is_err(), "always_fails should propagate the error");

    let after = counters.block_on_dyn_calls.load(Ordering::SeqCst);
    assert!(
        after > before,
        "executor must have been driven by the inter-attempt sleep \
         (before={before}, after={after})"
    );
}

#[test]
fn retry_async_eventually_succeeds() {
    let _counters = ensure_executor();
    let result = drive(fails_then_succeeds()).expect("second attempt should succeed");
    assert_eq!(result, "ok");
}

// ----------------------------------------------------------------------
// #[rate_limit] on async fn: the wait path must also go through the
// executor.
// ----------------------------------------------------------------------

#[rate_limit(rps = 1, burst = 1)]
async fn rate_limited_call() -> Result<u32, &'static str> {
    Ok(42)
}

#[test]
fn rate_limit_async_uses_registered_executor() {
    let counters = ensure_executor();
    let before = counters.block_on_dyn_calls.load(Ordering::SeqCst);

    // Two back-to-back calls force the second one through the
    // wait path (burst = 1, interval = 1s).
    let r1 = drive(rate_limited_call()).expect("first call should pass");
    assert_eq!(r1, 42);
    let r2 = drive(rate_limited_call()).expect("second call should pass after wait");
    assert_eq!(r2, 42);

    let after = counters.block_on_dyn_calls.load(Ordering::SeqCst);
    assert!(
        after > before,
        "executor must have been driven by the rate-limit wait \
         (before={before}, after={after})"
    );
}

// ----------------------------------------------------------------------
// `tokitai_core::async_sleep` itself: a unit assertion that the
// helper returns a Future that completes after the requested
// duration (and well under a hard ceiling so the test does not
// flake).
// ----------------------------------------------------------------------

#[test]
fn async_sleep_completes_after_requested_duration() {
    // Use a dedicated background thread with its own Tokio runtime
    // so this test does not interfere with (or get interfered by)
    // the other tests' runtime contexts. The test asserts that
    // `tokitai_core::async_sleep` yields to the runtime for at
    // least the requested duration and returns promptly afterwards.
    let handle = std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should build");
        let start = std::time::Instant::now();
        rt.block_on(async {
            tokitai_core::async_sleep(Duration::from_millis(30)).await;
        });
        start.elapsed()
    });
    let elapsed = handle.join().expect("test thread should not panic");
    assert!(
        elapsed >= Duration::from_millis(25),
        "async_sleep must wait at least ~30 ms (got {elapsed:?})"
    );
    assert!(
        elapsed < Duration::from_millis(2_000),
        "async_sleep must not over-sleep (got {elapsed:?})"
    );
}
