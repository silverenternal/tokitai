//! Build script for `tokitai-macros`.
//!
//! Two responsibilities:
//!
//! 1. **Quiet by default in tests.** The `#[tool]` macro can emit
//!    helpful warnings (deprecated methods missing `replaced_by`,
//!    `Option` parameters without defaults, ...) but those warnings
//!    pollute test output. We forward `TOKITAI_QUIET=1` to the
//!    proc-macro at compile time so test builds stay clean. End-user
//!    crates that *want* the warnings set `TOKITAI_QUIET=0` (or the
//!    user-facing crate `tokitai` re-exports the knob).
//!
//! 2. **Forward `TOKITAI_PROFILE`.** T-011 lets users opt into
//!    per-`#[tool]`-impl-block compile-time profiling. The macro
//!    emits a `cargo:warning=impl ... -> ... ms=...` line for each
//!    impl block. We forward `TOKITAI_PROFILE` into the macro's
//!    compile environment so the `option_env!("TOKITAI_PROFILE")`
//!    probe inside the macro source lights up when the consumer
//!    crate sets the env var.

fn main() {
    // Suppress tokitai macro warnings in test builds so the test
    // log stays focused on actual test failures.
    println!("cargo:rustc-env=TOKITAI_QUIET=1");

    // T-011: forward the profile knob. The consumer crate sets
    // `TOKITAI_PROFILE=1` in its environment; we re-export it as a
    // compile-time env var the macro can read via `option_env!`.
    // We do not set it to "1" unconditionally because users on
    // the default path would otherwise pay the cost of building
    // the profiling code path inside the macro.
    if let Ok(value) = std::env::var("TOKITAI_PROFILE") {
        if !value.is_empty() {
            // Tell rustc to expose this env var to the proc-macro
            // at compile time. We pass the literal value through
            // so users can encode a non-binary "profile mode"
            // (e.g. `TOKITAI_PROFILE=budget` for T-014) and the
            // macro can match on the string.
            println!("cargo:rustc-env=TOKITAI_PROFILE={}", value);
        }
    }
}
