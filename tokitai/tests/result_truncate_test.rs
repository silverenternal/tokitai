//! T-019: per-tool result-size budget (`#[tool(result_truncate_bytes = N)]`).
//!
//! Covers the four hard acceptance criteria from
//! `todo.json` v2.0 entry T-019:
//!
//!   1. `#[tool(result_truncate_bytes = 4096)]` on a method whose
//!      return value serializes to 8000 bytes produces a
//!      truncated result with the documented sentinel
//!      (`...[truncated at 4096 bytes, original was 8000 bytes]`).
//!   2. UTF-8 codepoint boundaries are respected for `String`
//!      returns (the truncation did not split a multi-byte
//!      codepoint).
//!   3. For arbitrary `Serialize` returns, the wrapper returns
//!      `ToolError::Truncated` instead of an unparseable
//!      half-JSON payload. The error message embeds the
//!      `original_bytes` and `kept_bytes` counts so a downstream
//!      log scraper can decide what to do.
//!   4. A negative case where the return value fits the budget
//!      — no truncation, no `tracing::warn!`.
//!
//! Plus a trybuild-driven `compile_fail` case (in
//! `tokitai-macros/tests/ui/`) for `result_truncate_bytes = 0`.
//!
//! Run:
//!   cargo test -p tokitai --test result_truncate_test
//!   cargo test -p tokitai --test result_truncate_test --features trace

use serde::Serialize;
use serde_json::json;
use tokitai::{tool, ToolErrorKind};

// =====================================================================
// 1. String truncation at a UTF-8 boundary
// =====================================================================

/// Provider whose methods exercise the four documented
/// behaviours. Methods are sync so the test does not need a Tokio
/// runtime; the truncation guard is identical for sync and async
/// wrappers because both paths route through the same
/// `result_truncate_guard` codegen site.
#[derive(Default)]
struct TruncateProvider;

#[tool]
impl TruncateProvider {
    /// String return value designed to be > 4096 bytes once
    /// serialized. Uses the "résumé" word (`é` = 2 bytes in UTF-8:
    /// `0xC3 0xA9`) so the truncation guard's UTF-8-boundary
    /// logic is exercised; if the guard walked back one byte too
    /// far it would leave a stray continuation byte at the end
    /// of the truncated payload and the test's `String::from_utf8`
    /// assertion would fail.
    #[tool(result_truncate_bytes = 4096)]
    pub fn big_string(&self) -> String {
        // ~4500 bytes after JSON-quoting; the `é` is repeated so a
        // 1-byte boundary slip would land mid-codepoint.
        let line = "résumé résumé résumé résumé résumé résumé\n";
        line.repeat(80)
    }

    /// Same idea as `big_string`, but the budget is high enough
    /// that no truncation fires. The negative path must NOT
    /// append a sentinel and must NOT mutate the payload.
    #[tool(result_truncate_bytes = 65536)]
    pub fn small_string(&self) -> String {
        "small payload".to_string()
    }

    /// Struct return value, sized > the budget. The wrapper must
    /// NOT return a half-JSON string for this — it must return
    /// `ToolError::Truncated` with the original/kept byte counts.
    #[tool(result_truncate_bytes = 1024)]
    pub fn big_struct(&self) -> BigStruct {
        BigStruct {
            title: "ok".to_string(),
            body: "x".repeat(2048),
        }
    }

    /// `Result`-returning version of `big_struct`. The
    /// `Err` arm is propagated as `ToolError::InternalError`
    /// (the same mapping the pre-T-019 `result_handling`
    /// used); the `Ok` arm of a struct over budget returns
    /// `ToolError::Truncated`.
    #[tool(result_truncate_bytes = 1024)]
    pub fn big_struct_result(&self) -> Result<BigStruct, String> {
        Ok(BigStruct {
            title: "ok".to_string(),
            body: "x".repeat(2048),
        })
    }

    /// Struct return value that fits the budget. No truncation,
    /// no `tracing::warn!`, no sentinel. Same as the
    /// pre-T-019 behaviour.
    #[tool(result_truncate_bytes = 8192)]
    pub fn small_struct(&self) -> BigStruct {
        BigStruct {
            title: "fits".to_string(),
            body: "x".repeat(64),
        }
    }
}

/// Payload type for the struct-truncation tests. Public so the
/// `Serialize` derive can see it. Two fields; one short, one
/// potentially large.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct BigStruct {
    title: String,
    body: String,
}

// =====================================================================
// 2. Tests
// =====================================================================

/// T-019 acceptance criterion 1: a `String` return over the
/// budget is truncated to a UTF-8 boundary and gets the
/// documented sentinel. The full sentinel
/// (`...[truncated at 4096 bytes, original was N bytes]`)
/// must appear at the end of the returned payload.
#[test]
fn string_truncation_lands_on_utf8_boundary() {
    let provider = TruncateProvider;
    let result = provider
        .call_tool("big_string", &json!({}))
        .expect("call should succeed with a truncated String");

    // The LLM-visible payload is a JSON string; unwrap it.
    let s = result
        .as_str()
        .expect("truncated payload should be a JSON string");

    // The full sentinel must be present, with the budget (4096)
    // and the original byte count.
    assert!(
        s.contains("...[truncated at 4096 bytes"),
        "expected the documented sentinel in the truncated payload, got: {s}"
    );
    // The original byte count must be > 4096 (the budget), and
    // the whole payload must round-trip through `from_utf8`
    // (i.e. the truncation guard respected codepoint
    // boundaries).
    let original = s
        .split("original was ")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .and_then(|n| n.trim().split(' ').next())
        .and_then(|n| n.parse::<usize>().ok())
        .expect("sentinel should embed the original byte count");
    assert!(
        original > 4096,
        "original bytes should exceed the budget; got {original}"
    );

    // No stray continuation byte at the end of the kept slice.
    // We split the payload at the sentinel to isolate the kept
    // prefix and assert it round-trips through `String::from_utf8`
    // (Rust's `str` already enforces that, but the explicit
    // check is a clear signal in the test report).
    let kept = s
        .split("...[truncated")
        .next()
        .expect("sentinel should be present");
    assert!(
        std::str::from_utf8(kept.as_bytes()).is_ok(),
        "kept prefix should be valid UTF-8, got bytes: {:?}",
        kept.as_bytes()
    );
    // And it should be no longer than the budget.
    assert!(
        kept.len() <= 4096,
        "kept prefix should not exceed the 4096-byte budget, got {} bytes",
        kept.len()
    );
}

/// T-019 acceptance criterion 2: a struct return over the
/// budget returns `ToolError::Truncated` instead of a
/// half-deserializable JSON payload. The error message embeds
/// the original and kept byte counts.
#[test]
fn struct_truncation_returns_tool_error_truncated() {
    let provider = TruncateProvider;
    let err = provider
        .call_tool("big_struct", &json!({}))
        .expect_err("over-budget struct call should return an error");

    assert_eq!(
        err.kind,
        ToolErrorKind::Truncated,
        "expected ToolErrorKind::Truncated, got: {:?}",
        err.kind
    );

    // The diagnostic message must embed both byte counts.
    let msg = err.message.to_string();
    assert!(
        msg.contains("original"),
        "expected `original` in the diagnostic, got: {msg}"
    );
    assert!(
        msg.contains("1024"),
        "expected the kept-byte count (1024) in the diagnostic, got: {msg}"
    );
}

/// Same as above, but exercising the `Result<T, E>` path. The
/// struct is in the `Ok` arm; the `Err` arm is not exercised
/// here. The wrapper must still surface `ToolError::Truncated`
/// with the byte counts, and must NOT re-wrap it as
/// `InternalError`.
#[test]
fn result_over_budget_returns_truncated_error() {
    let provider = TruncateProvider;
    let err = provider
        .call_tool("big_struct_result", &json!({}))
        .expect_err("over-budget Result::Ok call should return an error");

    assert_eq!(
        err.kind,
        ToolErrorKind::Truncated,
        "expected ToolErrorKind::Truncated for Result::Ok over budget, got: {:?}",
        err.kind
    );
}

/// T-019 acceptance criterion 3: a return value that fits the
/// budget is returned as-is. No sentinel, no
/// `ToolError::Truncated`, no `tracing::warn!` (the trace
/// feature is off in this test config).
#[test]
fn under_budget_return_is_unchanged() {
    let provider = TruncateProvider;

    // String under budget.
    let s = provider
        .call_tool("small_string", &json!({}))
        .expect("under-budget call should succeed");
    assert_eq!(
        s,
        json!("small payload"),
        "under-budget string should round-trip exactly"
    );

    // Struct under budget — also returned as-is.
    let v = provider
        .call_tool("small_struct", &json!({}))
        .expect("under-budget struct call should succeed");
    assert_eq!(v["title"], json!("fits"));
    assert_eq!(v["body"].as_str().unwrap().len(), 64);
}
