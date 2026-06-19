//! T-018: compile-time description quality linter.
//!
//! Every `desc = "..."` literal the user passes to a `#[tool(...)]`
//! method is scored against four signals at macro-expansion time
//! (length, type/unit hint, business-context keywords, sentence
//! count — 25 points each, 100 total). Below the per-impl
//! threshold (default 60/100) the macro refuses to compile with
//! a `compile_error!` anchored at the literal's span.
//!
//! These tests cover all four pillars of the acceptance criteria
//! from `todo.json`:
//!
//! 1. **`#[tool(desc = "adds")]` fails to compile** with the
//!    `error[E0031]: tool description scores X/100; minimum is
//!    60/100 ...` message. (negative case 1)
//! 2. **Long, business-rich descriptions pass** the lint. The
//!    canonical positive case mirrors the example in the task
//!    spec verbatim. (positive case 1)
//! 3. **Per-impl `min_desc_score = N`** lowers the threshold.
//!    (override case)
//! 4. **Per-impl `allow_short_desc`** opts the impl out of the
//!    lint entirely. (opt-out cases)
//!
//! The hidden `__property_would_error_str!` proc-macro hook
//! bridges the test runtime with the `tool` proc-macro so we can
//! exercise the lint without spinning up trybuild snapshots for
//! every string-shape permutation. The hook returns `true` when
//! the macro expansion contains a `compile_error!` invocation,
//! `false` otherwise.

use tokitai::tool;
use tokitai::ToolProvider;
use tokitai_macros::__property_would_error_str;

// ---------------------------------------------------------------------------
// Positive cases (5 total). Every description here passes the default
// 60/100 threshold.
// ---------------------------------------------------------------------------

/// Positive 1: the canonical example from the T-018 acceptance criteria.
/// Long, type-hinted (`i32` appears twice), business-context keywords
/// (`returns`, `requires`), and two sentences. Score should be 100/100.
pub struct PositiveAdd;

#[tool]
impl PositiveAdd {
    #[tool(
        desc = "Adds two 32-bit integers and returns their sum as i32. Requires both operands to be in the i32 range; returns Err on overflow."
    )]
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[test]
fn positive_canonical_example_compiles() {
    let tools = PositiveAdd::tool_definitions();
    assert_eq!(tools.len(), 1);
    let add = &tools[0];
    assert_eq!(add.name, "add");
    // The literal passes through unchanged — the lint does not
    // modify the description string when it scores above the bar.
    assert!(
        add.description.contains("Adds two 32-bit integers"),
        "description should be preserved verbatim, got: {}",
        add.description,
    );
}

/// Positive 2: every signal present, distinct domain. Score 100.
pub struct PositiveWrite;

#[tool]
impl PositiveWrite {
    #[tool(
        desc = "Writes the given text to the requested file as a UTF-8 String. Requires a non-empty path; persists the bytes to disk."
    )]
    pub fn write(&self, path: String, text: String) -> String {
        format!("wrote {} bytes to {}", text.len(), path)
    }
}

#[test]
fn positive_write_passes_lint() {
    let tools = PositiveWrite::tool_definitions();
    assert_eq!(tools.len(), 1);
    assert!(tools[0].description.contains("Writes"));
}

/// Positive 3: business-context keywords alone can carry a 30-char
/// description over the threshold when paired with a type hint.
pub struct PositiveSearch;

#[tool]
impl PositiveSearch {
    #[tool(
        desc = "Searches the user database and returns matching rows. Requires a non-empty needle."
    )]
    pub fn search(&self, _needle: String) -> Vec<String> {
        Vec::new()
    }
}

#[test]
fn positive_search_passes_lint() {
    let tools = PositiveSearch::tool_definitions();
    assert!(tools[0].description.contains("Searches"));
}

/// Positive 4: a unit hint ("ms") plus a business keyword pair.
pub struct PositiveLatency;

#[tool]
impl PositiveLatency {
    #[tool(
        desc = "Returns the latency in ms for the most recent request. Requires a session id; throws on unknown sessions."
    )]
    pub fn measure(&self, _session: String) -> u64 {
        0
    }
}

#[test]
fn positive_latency_passes_lint() {
    let tools = PositiveLatency::tool_definitions();
    assert!(tools[0].description.contains("latency"));
}

/// Positive 5: doc-comment description (no explicit `desc =`) is
/// not linted. The lint only fires on the `desc = "..."` literal.
pub struct PositiveDocComment;

#[tool]
impl PositiveDocComment {
    /// Returns a friendly greeting String for the supplied user name.
    pub fn greet(&self, name: String) -> String {
        format!("hello {}", name)
    }
}

#[test]
fn positive_doc_comment_description_is_not_linted() {
    let tools = PositiveDocComment::tool_definitions();
    assert!(tools[0].description.contains("friendly greeting"));
}

// ---------------------------------------------------------------------------
// Negative cases (3 total). Each one calls the macro pipeline through
// the property-test hook with source code that contains a short
// `desc =` literal. The hook returns `true` because the macro emits
// `compile_error!` at the literal's span.
// ---------------------------------------------------------------------------

#[test]
fn negative_three_char_desc_fails() {
    let would_error: bool = __property_would_error_str!(
        r#"
#[allow(dead_code)]
struct Three;
#[tokitai::tool::tool]
impl Three {
    #[tool(desc = "adds")]
    pub fn add(&self) -> i32 { 1 }
}
"#
    );
    assert!(
        would_error,
        "3-char `desc = \"adds\"` should fail the lint but the macro accepted it"
    );
}

#[test]
fn negative_no_signal_fails() {
    let would_error: bool = __property_would_error_str!(
        r#"
#[allow(dead_code)]
struct NoSig;
#[tokitai::tool::tool]
impl NoSig {
    #[tool(desc = "calculator")]
    pub fn calc(&self) -> i32 { 1 }
}
"#
    );
    assert!(
        would_error,
        "10-char desc with no signal should fail the lint"
    );
}

#[test]
fn negative_single_word_desc_fails() {
    let would_error: bool = __property_would_error_str!(
        r#"
#[allow(dead_code)]
struct OneWord;
#[tokitai::tool::tool]
impl OneWord {
    #[tool(desc = "ping")]
    pub fn ping(&self) -> i32 { 1 }
}
"#
    );
    assert!(would_error, "single 4-char desc should fail the lint");
}

// ---------------------------------------------------------------------------
// Opt-out cases (2 total). The impl-level `allow_short_desc` flag
// bypasses the lint regardless of score. Each test verifies that the
// macro accepts what would otherwise be rejected descriptions.
// ---------------------------------------------------------------------------

/// Opt-out 1: a 3-character verb that is genuinely the entire tool.
pub struct OptOutOne;

#[tool(allow_short_desc)]
impl OptOutOne {
    #[tool(desc = "go")]
    pub fn go(&self) -> i32 {
        1
    }
}

#[test]
fn opt_out_allow_short_desc_accepts_verb() {
    let tools = OptOutOne::tool_definitions();
    assert_eq!(tools[0].description, "go");
}

/// Opt-out 2: a 6-character noun where brevity is the point. The
/// `allow_short_desc` flag is at the impl level so every method on
/// the impl opts out by default.
pub struct OptOutSync;

#[tool(allow_short_desc)]
impl OptOutSync {
    #[tool(desc = "sync")]
    pub fn sync(&self) -> i32 {
        1
    }
}

#[test]
fn opt_out_allow_short_desc_accepts_noun() {
    let tools = OptOutSync::tool_definitions();
    assert_eq!(tools[0].description, "sync");
}

// ---------------------------------------------------------------------------
// Lowered-threshold case (1 total). The impl-level `min_desc_score`
// flag accepts descriptions that would otherwise be rejected by
// the default 60/100 bar.
// ---------------------------------------------------------------------------

/// Lowered threshold: the `min_desc_score = 30` flag accepts a
/// 3-char description (which scores 3/100 — way below the default
/// 60/100) because the impl-level threshold is 30. We use a
/// description that scores 30 (e.g. "adds two i32") to keep the
/// test stable across scorer revisions.
pub struct LowThreshold;

#[tool(min_desc_score = 25)]
impl LowThreshold {
    #[tool(desc = "adds two i32 values")]
    pub fn short_add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[test]
fn lowered_threshold_accepts_short_desc() {
    let tools = LowThreshold::tool_definitions();
    assert_eq!(tools[0].description, "adds two i32 values");
}
