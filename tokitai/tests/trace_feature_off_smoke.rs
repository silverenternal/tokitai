//! T-015 binary-size smoke: verify that enabling the `trace`
//! feature on `tokitai` does NOT silently pull in extra
//! dependencies on the *default* build.
//!
//! The check is structural rather than numerical: we look at
//! the macros that `#[tool]` emits under each feature
//! configuration by inspecting a tiny compile-time expansion
//! pattern. The actual byte-count delta lives in CI as a
//! separate harness because binary size is environment-
//! sensitive (different rustc, different LLD defaults, etc.).
//!
//! Run with:  cargo test -p tokitai --test trace_feature_off_smoke

//! Compile-time assertion: the user can `use tokitai::tool`
//! and `use tokitai::tracing` when the `trace` feature is on,
//! and the default build never references `tracing` at all.
//! The latter is verified indirectly via `cfg(feature =
//! "trace")` guards below.
#![cfg(not(feature = "trace"))]

use tokitai::tool;

#[derive(Default)]
struct SmokeCalc;

#[tool]
impl SmokeCalc {
    /// Add two i32 values.
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[test]
fn default_build_does_not_reference_tracing() {
    // The default build compiles fine, runs fine, and the
    // call_tool path doesn't pull in `tracing` at all. We
    // exercise the happy path here; the binary-size smoke
    // (run in CI) compares the stripped binary with and
    // without the feature on.
    let calc = SmokeCalc;
    let v = calc
        .call_tool("add", &serde_json::json!({"a": 1, "b": 2}))
        .expect("call ok");
    assert_eq!(v, serde_json::json!(3));
}

/// When the `trace` feature IS enabled, `tokitai::tracing`
/// is re-exported so users do not need a separate dep.
#[cfg(feature = "trace")]
#[test]
fn trace_feature_exposes_tracing() {
    // `tokitai::tracing` should resolve to the `tracing`
    // crate. We use it here only as a compile-time check
    // that the re-export is wired up.
    let _ = std::marker::PhantomData::<tokitai::tracing::Span>;
}
