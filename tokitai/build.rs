//! Build script for `tokitai`.
//!
//! Forwards compile-time env vars that the `#[tool]` macro reads
//! via `option_env!`. The macro itself lives in `tokitai-macros`
//! which has its own `build.rs`; this script complements that by
//! setting the same flags when the consumer-side crate enables
//! the matching `tokitai` features.
//!
//! Currently:
//!
//! - `trace` feature: forwards `TOKITAI_TRACE=1` so the macro
//!   emits `#[tracing::instrument(...)]` on every generated
//!   `__call_*` wrapper. Without this script, a consumer who
//!   enabled the `trace` feature would still see the macro
//!   behave as if the feature were off — because the macro's
//!   `option_env!` reads the env var, not the cargo feature.

fn main() {
    // T-015: when the consumer-side `trace` feature is on,
    // forward `TOKITAI_TRACE=1` to the macro's compile
    // environment so `option_env!("TOKITAI_TRACE")` inside
    // `tokitai-macros` resolves to `Some("1")` instead of
    // `None`.
    #[cfg(feature = "trace")]
    {
        // Only set the var if the consumer hasn't already
        // pinned a value via the env var (so
        // `TOKITAI_TRACE=` still works as an explicit kill
        // switch for users who want to compile with the
        // `trace` feature but disable instrumentation).
        if std::env::var("TOKITAI_TRACE").is_err() {
            println!("cargo:rustc-env=TOKITAI_TRACE=1");
        }
    }

    // Suppress macro warning output that would otherwise
    // pollute test logs. Consumers that want to see the
    // warnings can set `TOKITAI_QUIET=0` themselves.
    println!("cargo:rustc-env=TOKITAI_QUIET=1");
}
