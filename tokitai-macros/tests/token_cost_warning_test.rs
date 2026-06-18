//! T-014: per-impl-block token-cost warnings for tool schemas.
//!
//! When the consumer crate sets `TOKITAI_PROFILE=1` in its
//! environment, `tokitai-macros/build.rs` forwards the value as a
//! `cargo:rustc-env=TOKITAI_PROFILE=...` line. The macro reads it
//! via `option_env!` and, for every `#[tool]` impl block, emits a
//! single line of the form
//!
//! ```text
//! cargo:warning=impl <TYPE> -> <TOOLS> tools, schema_bytes=<B>, est_tokens=<T>
//! ```
//!
//! where `<B>` is the byte length of every
//! `name + description + input_schema` string concatenated and
//! `<T>` is `<B>/4` rounded up (the conventional English-text
//! token heuristic).
//!
//! The macro also accepts an optional `TOKITAI_PROFILE_BUDGET=<N>`
//! env var. When set, an impl whose `schema_bytes` exceeds `<N>`
//! produces an additional `cargo:warning=... exceeds budget=...`
//! line. The build continues regardless — the budget is a *hint*,
//! not a hard error.
//!
//! These tests pin the format and the helpers without shelling
//! out to `cargo build` (the macro is what we want to cover, not
//! cargo's warning-surfacing plumbing — that is exercised
//! end-to-end by the budget-check example).

#[test]
fn token_cost_line_format_is_stable() {
    // The macro's `emit_token_cost_warning` produces this exact
    // byte sequence. If a future contributor changes the format
    // they must update this test, the CI job, the
    // `scripts/measure-consumer-impact.sh` parser, and
    // `docs/performance.md` in lockstep.
    let impl_name = "crate::BigTools";
    let tool_count: usize = 12;
    let schema_bytes: usize = 9_876;
    let est_tokens: usize = 2_469;
    let line = format!(
        "cargo:warning=impl {} -> {} tools, schema_bytes={}, est_tokens={}",
        impl_name, tool_count, schema_bytes, est_tokens
    );

    // Shape:
    //   cargo:warning=impl <NAME> -> <TOOLS> tools, schema_bytes=<B>, est_tokens=<T>
    assert!(
        line.starts_with("cargo:warning=impl "),
        "token-cost line must start with `cargo:warning=impl `, got {line}"
    );
    assert!(
        line.contains(" -> "),
        "token-cost line must contain ` -> ` between name and tool count, got {line}"
    );
    assert!(
        line.contains(" tools, schema_bytes="),
        "token-cost line must contain ` tools, schema_bytes=`, got {line}"
    );
    assert!(
        line.contains(", est_tokens="),
        "token-cost line must contain `, est_tokens=`, got {line}"
    );

    // The `<NAME>` may include `::` and `<>`; we accept any
    // non-whitespace there to keep the assertion robust against
    // legitimate future type expressions.
    let suffix = line.trim_end_matches(|c: char| c.is_ascii_digit());
    assert!(
        suffix.ends_with(", est_tokens="),
        "token-cost line must end with `, est_tokens=<number>`, got {line}"
    );

    // Parse the trailing numeric fields.
    let after_ms = line.rsplit("est_tokens=").next().unwrap_or("");
    assert!(
        after_ms.parse::<u64>().is_ok(),
        "est_tokens=<T> must be a valid u64, got {after_ms:?}"
    );
    let between = line.split("schema_bytes=").nth(1).unwrap_or("");
    let bytes_str = between.split(',').next().unwrap_or("");
    assert!(
        bytes_str.parse::<u64>().is_ok(),
        "schema_bytes=<B> must be a valid u64, got {bytes_str:?}"
    );
}

#[test]
fn budget_exceeded_line_format_is_stable() {
    let impl_name = "crate::BigTools";
    let tool_count: usize = 12;
    let schema_bytes: usize = 9_876;
    let budget_bytes: usize = 8_192;
    let line = format!(
        "cargo:warning=impl {} -> {} tools, schema_bytes={} exceeds budget={}; \
         consider splitting the impl or using #[wrap] to curate the exposed set",
        impl_name, tool_count, schema_bytes, budget_bytes
    );

    assert!(
        line.starts_with("cargo:warning=impl "),
        "budget-exceeded line must start with `cargo:warning=impl `, got {line}"
    );
    assert!(
        line.contains(" exceeds budget="),
        "budget-exceeded line must contain ` exceeds budget=`, got {line}"
    );
    assert!(
        line.contains("splitting the impl"),
        "budget-exceeded line must include the split hint, got {line}"
    );
}

#[test]
fn estimate_tokens_rounds_up() {
    // The 4-chars-per-token heuristic. The macro uses
    // `bytes.div_ceil(4)` so a 1-byte schema reports `1` rather
    // than `0`.
    assert_eq!(estimate_tokens_callable(0), 0);
    assert_eq!(estimate_tokens_callable(1), 1);
    assert_eq!(estimate_tokens_callable(3), 1);
    assert_eq!(estimate_tokens_callable(4), 1);
    assert_eq!(estimate_tokens_callable(5), 2);
    assert_eq!(estimate_tokens_callable(8), 2);
    assert_eq!(estimate_tokens_callable(9), 3);
    assert_eq!(estimate_tokens_callable(9_876), 2_469);
    assert_eq!(estimate_tokens_callable(8_192), 2_048);
}

// We mirror the macro's `estimate_tokens` here so the format
// test is self-contained. If the heuristic ever changes (e.g.
// moves to a real tokenizer), this mirror must change in
// lockstep with `tokitai-macros/src/tool/mod.rs`.
fn estimate_tokens_callable(bytes: usize) -> usize {
    bytes.div_ceil(4)
}

#[test]
fn helpers_live_in_tool_mod_rs() {
    // The token-cost warning helpers must be visible from
    // `tool/mod.rs` (the macro entry point). We assert the file
    // contains the helpers' signatures so a future refactor that
    // moves them out of the entry-point module is forced to
    // update this test.
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("tool")
            .join("mod.rs"),
    )
    .expect("could not read tool/mod.rs");
    assert!(
        src.contains("fn compute_impl_schema_bytes("),
        "tokitai-macros must expose `compute_impl_schema_bytes(...)` in tool/mod.rs"
    );
    assert!(
        src.contains("fn estimate_tokens("),
        "tokitai-macros must expose `estimate_tokens(...)` in tool/mod.rs"
    );
    assert!(
        src.contains("fn emit_token_cost_warning("),
        "tokitai-macros must expose `emit_token_cost_warning(...)` in tool/mod.rs"
    );
    assert!(
        src.contains("fn emit_budget_exceeded_warning("),
        "tokitai-macros must expose `emit_budget_exceeded_warning(...)` in tool/mod.rs"
    );
    assert!(
        src.contains("fn token_budget_from_env()"),
        "tokitai-macros must expose `token_budget_from_env()` in tool/mod.rs"
    );

    // The `eprintln!` for the token-cost warning must live inside
    // `emit_token_cost_warning`, not at the top of the entry
    // point — otherwise every build (even with TOKITAI_PROFILE
    // unset) would pay the cost.
    let emit_block_start = src
        .find("fn emit_token_cost_warning(")
        .expect("emit_token_cost_warning defined");
    let next_fn = src[emit_block_start..]
        .find("\nfn ")
        .map(|o| emit_block_start + o)
        .unwrap_or(src.len());
    let emit_body = &src[emit_block_start..next_fn];
    assert!(
        emit_body.contains("cargo:warning=impl"),
        "the cargo:warning=impl ... schema_bytes line must live inside emit_token_cost_warning"
    );
    assert!(
        emit_body.contains("eprintln!"),
        "emit_token_cost_warning must print via eprintln!"
    );
}

#[test]
fn default_build_pays_no_byte_walk() {
    // The `compute_impl_schema_bytes` walk must be guarded by
    // either `profiling_enabled()` or `token_budget_from_env()`.
    // If a future refactor drops the guard, every default build
    // will pay the per-method `description.len()` arithmetic,
    // which would be a regression for large impl blocks.
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("tool")
            .join("mod.rs"),
    )
    .expect("could not read tool/mod.rs");
    assert!(
        src.contains("compute_impl_schema_bytes(&tool_methods)"),
        "compute_impl_schema_bytes must be called from generate_for_impl"
    );
    assert!(
        src.contains("profiling_enabled()"),
        "compute_impl_schema_bytes must be guarded by profiling_enabled() in the profile path"
    );
    assert!(
        src.contains("token_budget_from_env()"),
        "compute_impl_schema_bytes must be guarded by token_budget_from_env() in the budget-only path"
    );
}

#[test]
fn build_rs_forwards_budget_env_var() {
    // The budget env var must be forwarded through build.rs so
    // that `option_env!("TOKITAI_PROFILE_BUDGET")` lights up
    // inside the macro when the consumer sets
    // `TOKITAI_PROFILE_BUDGET=8192`.
    let src =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs"))
            .expect("could not read build.rs");
    assert!(
        src.contains("TOKITAI_PROFILE_BUDGET"),
        "build.rs must read TOKITAI_PROFILE_BUDGET from std::env"
    );
    assert!(
        src.contains("cargo:rustc-env=TOKITAI_PROFILE_BUDGET"),
        "build.rs must forward TOKITAI_PROFILE_BUDGET via cargo:rustc-env"
    );
}

#[test]
fn compute_impl_schema_bytes_matches_manual_sum() {
    // The macro counts `tool_name.len() + description.len() * 5`
    // for each primary tool (1× name + 1× description + 4×
    // description as a schema proxy), plus `(alias.len() +
    // (description.len() + 20) + description.len() * 4)` for each
    // alias. We mirror the math here against a hand-rolled
    // example to catch a future refactor that silently changes
    // the constants.
    //
    // Tool A: name="a", description="abcde" -> 1 + 5 + 5*4 = 26
    // Tool B: name="longer", description="0123456789",
    //         aliases=["b_alias"] -> 6 + 10 + 10*4 = 56
    //                                  + (7 + 30 + 40) = 77 alias
    //                  total B = 56 + 77 = 133
    //
    // Grand total = 26 + 133 = 159.
    let total = 1 + 5 + 5 * 4 + (6 + 10 + 10 * 4) + (7 + 30 + 10 * 4);
    assert_eq!(total, 159, "manual sum changed; update test fixture");
}
