//! T-034 (macro side): compile-time LLM-integration hooks.
//!
//! This module is the bridge between the `tokitai-llm` binary crate
//! (which lives next to `tokitai` in the workspace) and the `#[tool]`
//! proc-macro. The macro pipeline exposes three integration points:
//!
//! 1. **Verification report cache** ([`emit_verify_warnings`]):
//!    `tokitai-llm verify` writes a JSON report of any schema
//!    findings to a path stored in the `TOKITAI_LLM_VERIFY_REPORT`
//!    env var. The macro reads that path at expansion time and
//!    surfaces every finding as a `cargo:warning=` line so the
//!    LLM-side lint participates in the normal Rust build log.
//!    When the env var is unset the function is a no-op - the
//!    default `cargo build` pays nothing.
//!
//! 2. **Cache-key embedding** ([`emit_cache_key_const`]): every
//!    `#[tool]` impl block may opt into emitting a compile-time
//!    `pub const __TOKITAI_LLM_CACHE_KEY: &str` whose value is a
//!    stable hash of the schema (the same string the
//!    `tokitai-llm` runtime cache would key on, modulo the hash
//!    function). Code that wants to ship the same key to both
//!    the macro and the runtime can compare equality at boot.
//!    Opt in with `#[tool(llm_cache_key = true)]`.
//!
//! 3. **LLM-aware dispatch hook** ([`llm_aware_wrapper`]): when
//!    `TOKITAI_LLM_HOOK=1` is set, the macro wraps every
//!    generated `__call_*` body in a `tracing` span that
//!    records the resolved schema name + cache key as
//!    structured fields. The hook is opt-in and requires the
//!    consumer to also enable the `trace` feature (T-015).
//!    When the env var is unset the helper returns `None` and
//!    the macro emits the standard wrapper unchanged.
//!
//! ## Design constraints
//!
//! - **Zero runtime cost on the default path.** Every helper
//!   short-circuits when its env var is unset. There is no
//!   `match` on a non-`Option`, no allocation, no `String::new()`.
//! - **Env-var driven, not feature-flag driven.** The `#[tool]`
//!   macro cannot see the consumer's feature flags at expansion
//!   time in a way that distinguishes a "LLM build" from a
//!   regular build (the consumer may use any combination of
//!   features). The build-script-forwarded env var pattern
//!   matches T-011 / T-014 / T-015 / T-022's existing convention.
//! - **No new top-level dependencies.** The macro crate already
//!   pulls in `serde` and `serde_json`; both are used here.
//!   `blake3` would be a 50 KB addition for one constant string;
//!   we use `std::hash::DefaultHasher` instead so the key format
//!   is reproducible across platforms (SipHasher13 with fixed
//!   keys `(0, 0)`).
//!
//! See `docs/AI_INTEGRATION.md` for the operator-facing runbook.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use proc_macro2::TokenStream;
use quote::quote;

use crate::tool::types::tool_method::ToolMethodInfo;

/// The default verification report filename the macro writes to
/// `OUT_DIR` when the consumer did not pass an explicit path via
/// `TOKITAI_LLM_VERIFY_REPORT`. The constant is `pub(crate)` so
/// the `build.rs` companion file can reference it from its
/// `cargo:rerun-if-changed=` directive without duplicating the
/// literal.
#[allow(dead_code)]
pub(crate) const DEFAULT_VERIFY_REPORT_NAME: &str = "tokitai_llm_verify.json";

/// Compile-time probe: is the LLM verification report cache path
/// known to the macro? The path is forwarded by `build.rs` from
/// `TOKITAI_LLM_VERIFY_REPORT` and may be either an absolute
/// filesystem path or a bare filename to be resolved against
/// `OUT_DIR`.
///
/// The probe is `option_env!` (not `std::env::var`) because the
/// value is baked into the macro at compile time - by the time a
/// proc-macro invocation runs, `std::env::var` of the host process
/// no longer reflects the cargo build environment that drove
/// this build.
fn verify_report_path() -> Option<&'static str> {
    let raw = option_env!("TOKITAI_LLM_VERIFY_REPORT")?;
    if raw.is_empty() {
        None
    } else {
        Some(raw)
    }
}

/// Compile-time probe: is the LLM dispatch hook enabled?
///
/// `TOKITAI_LLM_HOOK=1` opts the macro into emitting an
/// LLM-aware wrapper that records the schema name + cache key
/// as structured `tracing` fields. Without the env var (the
/// default) the macro emits the standard wrapper unchanged.
fn llm_hook_enabled() -> bool {
    option_env!("TOKITAI_LLM_HOOK").is_some_and(|v| !v.is_empty())
}

/// One entry in the verification report. The on-disk JSON shape
/// is intentionally flat (no nested metadata) so a hand-written
/// fixture in the tests/ directory is trivial to maintain.
///
/// ```json
/// {
///   "version": 1,
///   "findings": [
///     { "tool": "Calculator.add", "code": "MCP-1", "message": "..." }
///   ]
/// }
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct VerifyReport {
    /// Schema version of the report. Currently always `1`.
    /// Bumped when the shape changes in a backwards-incompatible
    /// way; the macro refuses to parse a future version it does
    /// not understand.
    #[serde(default = "default_version")]
    pub version: u32,
    /// Findings emitted by `tokitai-llm verify` against the
    /// impl block's tool schemas. Empty list means clean.
    #[serde(default)]
    pub findings: Vec<VerifyFinding>,
}

fn default_version() -> u32 {
    1
}

/// A single schema-verification finding. The `code` is a stable
/// identifier the operator can grep for (e.g. `MCP-1` for
/// "schema node has no `type` keyword"); `tool` is the
/// `Type.method` pair the LLM-side linter attributed the finding
/// to (or empty when the finding is impl-block-level).
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct VerifyFinding {
    /// Tool the finding was attributed to (`Type::method`). May be
    /// empty for impl-block-level findings.
    pub tool: String,
    /// Stable finding code from `tokitai-llm verify` (e.g. `MCP-1`).
    pub code: String,
    /// Human-readable description of the issue.
    pub message: String,
}

/// Parse a verification report from a JSON string. Returns
/// `Ok(report)` for valid input, `Err(_)` for malformed JSON or
/// an unknown schema version. The parser is intentionally
/// permissive on extra fields (the report may grow over time)
/// but strict on the schema `version`.
fn parse_report(text: &str) -> Result<VerifyReport, String> {
    let report: VerifyReport =
        serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
    if report.version != 1 {
        return Err(format!(
            "unsupported report version {} (this macro knows version 1)",
            report.version
        ));
    }
    Ok(report)
}

/// Emit `cargo:warning=` lines for every entry in the LLM
/// verification report cache. Called from
/// `generate_for_impl` once per `#[tool]` impl
/// block when `TOKITAI_LLM_VERIFY_REPORT` is set.
///
/// The function never panics: a malformed report degrades to a
/// single `cargo:warning=` line saying so, and an empty findings
/// list emits nothing (the cache hit means the schema is clean).
///
/// Returns `true` when the report was successfully read AND had
/// at least one finding; the caller uses the return value to
/// track profile-style counters in the T-011 path.
pub(crate) fn emit_verify_warnings(impl_name: &str) -> bool {
    // Fast-path: no env var => no work. The default `cargo
    // build` path must pay exactly zero for the LLM hooks -
    // not even a `DefaultHasher::new()` call.
    let path = match verify_report_path() {
        Some(p) => p,
        None => return false,
    };

    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            // The build-script may have set the env var but the
            // file may not exist yet (the `tokitai-llm verify`
            // step is a separate cargo invocation). Surface a
            // single warning so the operator can see the missing
            // file without breaking the build.
            eprintln!(
                "cargo:warning=impl {} -> tokitai-llm verify report at `{}` is not readable: {}",
                impl_name, path, e
            );
            return false;
        }
    };

    let report = match parse_report(&text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "cargo:warning=impl {} -> tokitai-llm verify report at `{}` could not be parsed: {}",
                impl_name, path, e
            );
            return false;
        }
    };

    let mut emitted = false;
    for f in &report.findings {
        // Attribute the warning to the impl block by name and to
        // the tool (when known). Operators can grep for either
        // piece of information.
        let tool_label = if f.tool.is_empty() {
            "(impl-level)".to_string()
        } else {
            f.tool.clone()
        };
        eprintln!(
            "cargo:warning=impl {} -> tokitai-llm verify: [{}] {}: {}",
            impl_name, f.code, tool_label, f.message
        );
        emitted = true;
    }
    emitted
}

/// Compute a stable cache key for an impl block's schema set.
///
/// The key is a 16-char hex string of `DefaultHasher` output
/// (SipHasher13) over the concatenated `(name, description)`
/// pairs of every tool. The hash is computed at proc-macro time
/// - no I/O, no allocations beyond the formatter.
///
/// Reproducibility: `DefaultHasher` is `SipHasher13` with fixed
/// keys `(0, 0)` on every platform Rust supports, so the same
/// schema produces the same key on every machine. The
/// `tokitai-llm` runtime cache uses blake3 (a different hash
/// function); the two are not interchangeable. The macro-side
/// key is intended for "is this the same schema as last build?"
/// comparisons inside Rust code, NOT for cross-language cache
/// lookups.
pub(crate) fn schema_cache_key(tool_methods: &[ToolMethodInfo]) -> String {
    let mut hasher = DefaultHasher::new();
    // Hash every tool's name + description. Order matters: the
    // slice is the source-level declaration order, which is
    // stable across rebuilds (the operator does not reorder
    // methods between `cargo build` runs in practice).
    for t in tool_methods {
        t.tool_name.hash(&mut hasher);
        // Separator so a tool named "abc" with description "def"
        // does not collide with a tool named "ab" with
        // description "cdef".
        0u8.hash(&mut hasher);
        t.description.hash(&mut hasher);
        0u8.hash(&mut hasher);
    }
    // Length-prefix the slice so reordering the methods between
    // builds produces a different key (the hasher alone is
    // order-sensitive, but the explicit length makes the
    // contract obvious to readers).
    tool_methods.len().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Render the `pub const __TOKITAI_LLM_CACHE_KEY: &str = "..."`
/// item for one impl block. Returns `None` when the impl did
/// not opt in via `#[tool(llm_cache_key = true)]` - the default
/// behaviour matches every other compile-time-only feature in
/// the macro (the cost of emitting one extra `const` per
/// `#[tool]` block is ~30 bytes of `.rodata`; we do not pay
/// that by default).
pub(crate) fn emit_cache_key_const(
    tool_methods: &[ToolMethodInfo],
    opt_in: bool,
) -> Option<TokenStream> {
    if !opt_in {
        return None;
    }
    let key = schema_cache_key(tool_methods);
    Some(quote! {
        #[doc(hidden)]
        pub const __TOKITAI_LLM_CACHE_KEY: &'static str = #key;
    })
}

/// Wrap a generated `__call_<method>` body in the LLM-aware
/// trace hook. Returns `Some(TokenStream)` when
/// `TOKITAI_LLM_HOOK=1` is set, `None` otherwise.
///
/// The hook emits a `tracing::info_span!("tokitai_llm_call",
/// tool = ..., cache_key = ...).entered()` at the top of the
/// body so downstream log aggregators (Datadog, Honeycomb, OTLP)
/// can group every call by the tool's stable cache key. The
/// hook requires the `tracing` crate at the consumer site -
/// same pattern as T-015.
///
/// The body the caller passes in is the *inner* body (the
/// resolved-args pattern match). We wrap, not replace, so
/// compile-time-error semantics from the regular path are
/// preserved.
#[allow(dead_code)]
pub(crate) fn llm_aware_wrapper(
    tool_name: &str,
    cache_key: &str,
    inner_body: TokenStream,
) -> Option<TokenStream> {
    if !llm_hook_enabled() {
        return None;
    }
    Some(quote! {
        let __tokitai_llm_span = ::tracing::info_span!(
            "tokitai_llm_call",
            tool = #tool_name,
            cache_key = #cache_key,
        );
        let __tokitai_llm_guard = __tokitai_llm_span.enter();
        #inner_body
    })
}

/// Return `true` when the LLM integration is enabled in any
/// form (verify-report path set OR hook env var set). Used by
/// `generate_for_impl` to skip the helper calls on the default
/// path so the macro expansion is allocation-free.
pub(crate) fn any_hook_enabled() -> bool {
    verify_report_path().is_some() || llm_hook_enabled()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::types::param::ParamInfo;
    use proc_macro2::Span;
    use syn::ReturnType;

    /// Build a minimal `ToolMethodInfo` for unit tests. The
    /// `#[allow(clippy::field_reassign_with_default)]` lint
    /// lets us write the test fixtures as a single block
    /// without repeating every field of the real struct.
    #[allow(clippy::field_reassign_with_default)]
    fn make_method(name: &str, desc: &str) -> ToolMethodInfo {
        let mut m = ToolMethodInfo {
            ident_span: Span::call_site(),
            name: name.to_string(),
            tool_name: name.to_string(),
            description: desc.to_string(),
            params: Vec::<ParamInfo>::new(),
            is_async: false,
            is_result: false,
            is_generic: false,
            deprecated: false,
            replaced_by: None,
            deprecated_note: None,
            deprecated_since: None,
            remove_in: None,
            version: None,
            visible: true,
            tags: Vec::new(),
            group: None,
            return_description: None,
            context: None,
            example_input: None,
            param_order: None,
            hidden_params: Vec::new(),
            example_output: None,
            return_type: ReturnType::Default,
            doc: None,
            alias: Vec::new(),
            usage_hint: None,
            allow: Vec::new(),
            cache: None,
            rate_limit: None,
            param_validations: Vec::new(),
            description_explicit: false,
            baked_examples: Vec::new(),
            desc_span: None,
            min_desc_score: None,
            allow_short_desc: false,
            allow_insecure_desc: false,
            allow_imperative_desc: false,
            desc_safety_scope: None,
            desc_blocklist: Vec::new(),
            result_truncate_bytes: None,
            since: None,
            until: None,
            requires: Vec::new(),
            requires_invalid: false,
            requires_invalid_span: None,
        };
        // `name` is what the macro would store for dispatch;
        // we keep them identical to avoid confusing the
        // hash-based cache key with the dispatch key.
        m.tool_name = name.to_string();
        m
    }

    #[test]
    fn parse_report_accepts_clean_fixture() {
        let text = r#"{"version":1,"findings":[]}"#;
        let report = parse_report(text).expect("clean fixture parses");
        assert_eq!(report.version, 1);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn parse_report_accepts_findings() {
        let text = r#"{
            "version": 1,
            "findings": [
                {"tool": "Calculator.add", "code": "MCP-1", "message": "no type"}
            ]
        }"#;
        let report = parse_report(text).expect("fixture with findings parses");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "MCP-1");
        assert_eq!(report.findings[0].tool, "Calculator.add");
    }

    #[test]
    fn parse_report_defaults_version_when_missing() {
        // The `version` field has `#[serde(default)]` so an
        // older report without the field still parses - the
        // default-1 path then matches the schema-1 contract.
        let text = r#"{"findings":[]}"#;
        let report = parse_report(text).expect("missing version defaults to 1");
        assert_eq!(report.version, 1);
    }

    #[test]
    fn parse_report_rejects_unknown_version() {
        let text = r#"{"version":99,"findings":[]}"#;
        let err = parse_report(text).expect_err("future version must reject");
        assert!(err.contains("unsupported report version"));
    }

    #[test]
    fn parse_report_rejects_garbage() {
        let text = "this is not json";
        let err = parse_report(text).expect_err("garbage must reject");
        assert!(err.contains("not valid JSON"));
    }

    #[test]
    fn schema_cache_key_is_stable() {
        // Same input -> same key. The default hasher is
        // deterministic across runs (SipHasher13 with fixed
        // keys (0,0)).
        let k1 = schema_cache_key(&[]);
        let k2 = schema_cache_key(&[]);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 16, "hex u64 is exactly 16 chars");
    }

    #[test]
    fn schema_cache_key_changes_with_input() {
        // Different input -> different key (with overwhelming
        // probability; SipHasher collisions on 64-bit space
        // are vanishingly rare for short inputs).
        let a = make_method("add", "Adds two integers");
        let b = make_method("sub", "Subtracts two integers");
        let k1 = schema_cache_key(&[a]);
        let k2 = schema_cache_key(&[b]);
        assert_ne!(k1, k2);
    }

    #[test]
    fn schema_cache_key_changes_with_reorder() {
        // The contract: reordering the methods produces a
        // different key. This catches the operator mistake of
        // swapping two method declarations and accidentally
        // hitting the same cache key on the runtime side.
        let a = make_method("add", "adds");
        let b = make_method("sub", "subs");
        let forward = schema_cache_key(&[a.clone(), b.clone()]);
        let reverse = schema_cache_key(&[b, a]);
        assert_ne!(forward, reverse, "reordering must change the cache key");
    }

    #[test]
    fn emit_cache_key_const_no_opt_in_returns_none() {
        // The default build never opts in; the helper must
        // return None so `generate_for_impl` does not emit the
        // const on the default path.
        let result = emit_cache_key_const(&[], false);
        assert!(result.is_none());
    }

    #[test]
    fn emit_cache_key_const_opt_in_emits_const() {
        let result = emit_cache_key_const(&[], true);
        let tokens = result.expect("opt-in returns Some");
        let rendered = tokens.to_string();
        assert!(
            rendered.contains("__TOKITAI_LLM_CACHE_KEY"),
            "rendered const must contain the symbol name, got: {}",
            rendered
        );
    }

    #[test]
    fn llm_aware_wrapper_returns_none_without_env_var() {
        // The macro source compiled without `TOKITAI_LLM_HOOK=1`
        // (the unit-test build is the default), so the helper
        // must return None.
        let result = llm_aware_wrapper("add", "deadbeef", quote! { 42 });
        assert!(result.is_none());
    }
}
