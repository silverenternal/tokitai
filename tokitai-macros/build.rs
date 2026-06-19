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

    // T-014: forward the optional schema-byte budget. The macro
    // reads `option_env!("TOKITAI_PROFILE_BUDGET")` and emits a
    // `cargo:warning=... exceeds budget=...` line per impl block
    // whose combined `name + description + input_schema` byte
    // count exceeds the threshold. The default build (no env
    // var) skips the budget gate entirely; setting
    // `TOKITAI_PROFILE_BUDGET=8192` (8 KB ≈ 2 000 tokens) is a
    // reasonable starting point for an OpenAI-shaped provider.
    //
    // The value is forwarded verbatim — even non-numeric strings
    // — so users can write `TOKITAI_PROFILE_BUDGET=64KB` as a
    // self-documenting form; the macro's `parse::<usize>` will
    // then fail to parse it and quietly disable the budget
    // check (this is the same behaviour as setting an empty
    // value). Users who want the gate *on* should stick to
    // a plain integer like `8192` or `65536`.
    if let Ok(value) = std::env::var("TOKITAI_PROFILE_BUDGET") {
        if !value.is_empty() {
            println!("cargo:rustc-env=TOKITAI_PROFILE_BUDGET={}", value);
        }
    }

    // T-015: forward the trace toggle. The macro reads
    // `option_env!("TOKITAI_TRACE")` and, when set, emits
    // `#[tracing::instrument(...)]` on every generated `__call_*`
    // wrapper. The default build (no env var) emits no
    // `tracing` calls so the binary size delta is exactly zero
    // (verified by the binary-size smoke in CI). The
    // end-user-facing crate `tokitai` exposes a `trace` feature
    // that wraps this knob; we forward the raw env var as well
    // so users who want to flip the switch without changing
    // their `Cargo.toml` can do so via `TOKITAI_TRACE=1`.
    if let Ok(value) = std::env::var("TOKITAI_TRACE") {
        if !value.is_empty() {
            println!("cargo:rustc-env=TOKITAI_TRACE={}", value);
        }
    }

    // T-015: when a downstream crate enables the `tokitai/trace`
    // feature (which cascades into the `tokitai-macros/trace`
    // feature), cargo sets `CARGO_FEATURE_TRACE=1` in this
    // build script's environment. Forward it to the macro's
    // compile environment so `option_env!("TOKITAI_TRACE")`
    // inside the macro lights up and `#[tracing::instrument]`
    // is emitted on every generated `__call_*` wrapper. This
    // means downstream consumers (test binaries, examples,
    // end-user crates) get spans emitted automatically whenever
    // they turn on the `trace` feature — no env-var
    // bookkeeping required.
    if std::env::var("TOKITAI_TRACE").is_err() && std::env::var("CARGO_FEATURE_TRACE").is_ok() {
        println!("cargo:rustc-env=TOKITAI_TRACE=1");
    }

    // T-022: forward the per-build adversarial-description
    // blocklist. The macro reads `option_env!("TOKITAI_DESC_BLOCKLIST")`
    // and matches every comma-separated phrase as a case-
    // insensitive substring. The default build (no env var)
    // skips the matcher entirely, so the only cost is one
    // `option_env!` probe at macro-expansion time per
    // `#[tool]` impl block. Setting
    // `TOKITAI_DESC_BLOCKLIST="ignore previous,system:,_test_"`
    // lets a security team extend the org-wide bad-pattern set
    // without rebuilding the macro crate. The plumbing mirrors
    // T-015's `TOKITAI_TRACE` and T-014's `TOKITAI_PROFILE_BUDGET`
    // exactly so operators learn one env-var convention.
    if let Ok(value) = std::env::var("TOKITAI_DESC_BLOCKLIST") {
        if !value.is_empty() {
            println!("cargo:rustc-env=TOKITAI_DESC_BLOCKLIST={}", value);
        }
    }
}
