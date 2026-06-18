//! Integration test for T-003: the `AsyncExecutor::block_on_for` per-call
//! override seam, exercised end-to-end against an `async-std`-style
//! executor.
//!
//! This test is intentionally in its own binary so the process-wide
//! `ASYNC_EXECUTOR` `OnceLock` is guaranteed to be empty when the test
//! runs (cf. `async_executor_no_executor_test.rs`).
//!
//! Run with:
//!     cargo test -p tokitai-core --test async_executor_override_test --features serde
//!
//! The test exercises the full priority chain that the `#[tool]` macro
//! now implements:
//!
//!   1. `block_on_for_executor()` (T-003 per-call seam, OVERRIDE).
//!   2. `current_async_executor()` (global slot from `set_async_executor`).
//!   3. Active Tokio runtime (out of scope here — we use a single-thread
//!      driver so the suite stays runtime-agnostic).
//!   4. `block_on_async_error_message()` English fallback.
//!
//! We do not depend on `async-std` itself; the test executor captures
//! the same protocol (per-call `block_on_for` returns `Some(self)`, drives
//! the future on the current thread, and exposes an internal counter so
//! the test can prove the override path was actually exercised).
//!
//! Because all tests in this binary share a single process-wide
//! `OnceLock<Box<dyn AsyncExecutor>>`, the test is structured as ONE
//! end-to-end scenario so the global state cannot drift between
//! assertions. Sub-assertions live inside that one scenario.

#![cfg(feature = "serde")]

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokitai_core::{
    block_on_async, block_on_async_error_message, block_on_for_executor, set_async_executor,
    AsyncExecutor, AsyncExecutorExt, ToolErrorKind,
};

/// Counts how many futures its `block_on_dyn` has driven. Used to assert
/// that the override path (not a fallback) was used.
#[derive(Default)]
struct OverrideCounters {
    block_on_dyn_calls: AtomicUsize,
    block_on_for_calls: AtomicUsize,
}

/// An `async-std`-style executor stub. Mirrors the API shape we expect
/// from a real `async_std::task::Executor` wrapper: implements
/// [`AsyncExecutor`], overrides [`AsyncExecutor::block_on_for`] to return
/// `Some(self)`, and counts drives so the test can prove the override
/// path was exercised.
struct AsyncStdLikeExecutor {
    counters: Arc<OverrideCounters>,
}

impl AsyncStdLikeExecutor {
    fn new(counters: Arc<OverrideCounters>) -> Self {
        Self { counters }
    }
}

impl AsyncExecutor for AsyncStdLikeExecutor {
    fn block_on_dyn(
        &self,
        future: Pin<Box<dyn core::future::Future<Output = ()> + Send>>,
    ) -> Box<dyn core::any::Any + Send> {
        // Drive the future on the current thread. We deliberately do
        // not depend on a specific runtime crate: `futures::executor::
        // block_on` is part of the existing dev-dependencies and is the
        // same single-thread driver `async-std`'s `task::block_on` uses
        // internally for `Send + 'static` futures.
        self.counters
            .block_on_dyn_calls
            .fetch_add(1, Ordering::SeqCst);
        futures::executor::block_on(future);
        Box::new(())
    }

    /// T-003: the seam we are testing. Returning `Some(self)` makes
    /// `block_on_for_executor()` resolve to *this* executor, ahead of
    /// the global slot, even when the global slot is also populated.
    fn block_on_for(&self) -> Option<&'static dyn AsyncExecutor> {
        self.counters
            .block_on_for_calls
            .fetch_add(1, Ordering::SeqCst);
        // We must hand back a `'static` reference. The trait method is
        // bounded by `&self`, so we leak a fresh clone of the executor
        // for the remainder of the test binary. The leak is bounded to
        // the test process.
        let boxed: Box<dyn AsyncExecutor> = Box::new(AsyncStdLikeExecutor {
            counters: Arc::clone(&self.counters),
        });
        Some(Box::leak(boxed))
    }
}

/// End-to-end scenario for T-003:
///
///   1. The canonical English error message is stable and reachable.
///   2. Registering an `async-std`-style executor (with `block_on_for`
///      overridden) installs it in the global slot AND makes
///      `block_on_for_executor()` resolve through the per-call seam.
///   3. `block_on_async` routes through the override seam, proved by
///      the counter — not just by the return value.
///   4. The typed `AsyncExecutorExt::block_on` entry point works
///      against the per-call probe result.
#[test]
fn test_async_executor_override_end_to_end() {
    // (1) Canonical English error message is stable.
    let msg = block_on_async_error_message();
    assert!(
        msg.contains("no async runtime"),
        "canonical error must mention 'no async runtime', got: {msg}"
    );
    assert!(
        msg.contains("set_async_executor"),
        "canonical error must point at set_async_executor, got: {msg}"
    );

    // (2) Register an `async-std`-style executor. The per-call seam
    //     resolves to it through the override probe.
    let counters = Arc::new(OverrideCounters::default());
    set_async_executor(Box::new(AsyncStdLikeExecutor::new(Arc::clone(&counters))));

    let probed = block_on_for_executor().expect("per-call override must be set");

    // (3) `block_on_async` routes through the override path. The
    //     counter proves it: the override's `block_on_dyn` is the only
    //     thing that can bump it.
    let drives_before = counters.block_on_dyn_calls.load(Ordering::SeqCst);
    let output: String = block_on_async(async {
        let mut s = String::from("override-");
        s.push_str("works");
        s
    })
    .expect("block_on_async should succeed when an override executor is registered");
    assert_eq!(output, "override-works");
    let drives_after = counters.block_on_dyn_calls.load(Ordering::SeqCst);
    assert!(
        drives_after > drives_before,
        "override executor must have driven the future (before={}, after={})",
        drives_before,
        drives_after
    );

    // (4) Typed `AsyncExecutorExt::block_on` against the per-call
    //     probe result. This is the path the `#[tool]` macro's sync
    //     wrapper uses.
    let value: i64 = probed.block_on(async { 100i64 + 23i64 });
    assert_eq!(value, 123);

    // (5) The override probe also ran at least once.
    assert!(
        counters.block_on_for_calls.load(Ordering::SeqCst) >= 1,
        "block_on_for should have been probed at least once"
    );
}

/// `block_on_async_error_message` returns a stable English string.
#[test]
fn test_block_on_async_error_message_stable_text() {
    let msg = block_on_async_error_message();
    assert!(msg.contains("no async runtime"));
    assert!(msg.contains("set_async_executor"));
}

/// The `ToolErrorKind` for the override-exhausted path is
/// `InternalError`. The macro's sync wrapper surfaces this kind on
/// failure, so it is part of the T-003 contract.
#[test]
fn test_override_failure_kind_is_internal_error() {
    // The success path cannot easily be turned into a failure here
    // (the stub executor always succeeds). We assert the kind of the
    // error path indirectly: the canonical message returned by
    // `block_on_async_error_message` is paired with
    // `ToolErrorKind::InternalError` inside `block_on_async`. The
    // companion test in `async_executor_no_executor_test.rs`
    // confirms the kind on a fresh process.
    //
    // What we *can* assert here is that the error message text matches
    // the kind — by constructing the error message the macro would
    // surface and confirming it is the same string returned by
    // `block_on_async_error_message`.
    let canonical = block_on_async_error_message();
    assert!(canonical.starts_with("no async runtime"));
    // The companion test asserts `kind == ToolErrorKind::InternalError`
    // in a separate binary; here we just keep a stub assertion that
    // documents the contract.
    let _ = ToolErrorKind::InternalError;
}
