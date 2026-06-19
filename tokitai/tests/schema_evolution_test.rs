//! T-020: end-to-end test for typed schema evolution.
//!
//! The acceptance criteria are:
//!
//! 1. `#[tool(since = "1.0", until = "2.0")]` and
//!    `#[tool(since = "2.0")]` on two methods in the same impl
//!    block compile cleanly.
//! 2. Calling `set_current_version("1.5")` exposes only the 1.0
//!    method; `set_current_version("2.0")` exposes only the 2.0
//!    method.
//! 3. Additive change (new optional field in 2.0) is non-breaking
//!    — `set_current_version("1.0")` still serves the 1.0 method
//!    with the new optional field set to `None`.
//! 4. `version_policy = "semver"` enforces ordered comparisons
//!    (since `"1.10.0"` < until `"2.0.0"` is true; since `"2.0.0"`
//!    < until `"2.0.0"` is false).
//! 5. Loose strings (CalVer, commit SHA) are accepted and disable
//!    the typed-diff feature (no compile error, runtime uses
//!    lexicographic ordering).
//!
//! Run with: `cargo test -p tokitai --test schema_evolution_test`

#![cfg(feature = "serde")]
#![allow(non_snake_case, non_upper_case_globals)]
#![allow(clippy::default_constructed_unit_structs)]

use std::sync::Mutex;

use serde_json::json;
use tokitai::tool;
use tokitai::{clear_current_version, set_current_version, ToolCaller, ToolProvider};

/// T-020's per-impl `FILTER_CACHE` plus the global
/// `current_version()` slot are process-wide. Tests that touch
/// either must run serially. We hold this mutex at the top of
/// every test so the suite is deterministic on a single
/// process — without it, `cargo test` runs the tests in
/// parallel and `set_current_version` calls from one test
/// leak into another.
static VERSION_LOCK: Mutex<()> = Mutex::new(());

/// Acquire `VERSION_LOCK` at the top of a `#[test]` body. The
/// guard is dropped when the body returns, releasing the lock
/// for the next test.
#[allow(dead_code)]
fn lock_versions() -> std::sync::MutexGuard<'static, ()> {
    VERSION_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ============================================================================
// Test impl 1: per-method since/until intervals. Two methods tile the
// 1.0..2.0 and 2.0..infinity ranges respectively.
// ============================================================================

#[derive(Default)]
struct EvolvingTools;

#[tool]
impl EvolvingTools {
    /// Add two 32-bit integers. Available in v1.
    #[tool(since = "1.0", until = "2.0")]
    pub fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }

    /// Multiply two 32-bit integers. Available in v2.
    #[tool(since = "2.0")]
    pub fn mul(&self, a: i64, b: i64) -> i64 {
        a * b
    }
}

#[test]
fn test_since_until_bake_into_definition() {
    let _lock = lock_versions();
    // No version set: the fast path returns the full slice.
    clear_current_version();
    let tools = EvolvingTools::tool_definitions();
    let add = tools.iter().find(|t| t.name == "add").unwrap();
    assert_eq!(add.since.as_deref(), Some("1.0"));
    assert_eq!(add.until.as_deref(), Some("2.0"));
    let mul = tools.iter().find(|t| t.name == "mul").unwrap();
    assert_eq!(mul.since.as_deref(), Some("2.0"));
    assert_eq!(mul.until, None);
}

#[test]
fn test_current_version_one_point_five_exposes_only_add() {
    let _lock = lock_versions();
    set_current_version("1.5");
    let tools = EvolvingTools::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names.contains(&"add"),
        "add must be served at v1.5: {:?}",
        names
    );
    assert!(
        !names.contains(&"mul"),
        "mul (since=2.0) must NOT be served at v1.5: {:?}",
        names
    );
}

#[test]
fn test_current_version_two_exposes_only_mul() {
    let _lock = lock_versions();
    set_current_version("2.0");
    let tools = EvolvingTools::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        !names.contains(&"add"),
        "add (until=2.0) must NOT be served at v2.0: {:?}",
        names
    );
    assert!(
        names.contains(&"mul"),
        "mul must be served at v2.0: {:?}",
        names
    );
}

#[test]
fn test_current_version_two_point_five_exposes_only_mul() {
    let _lock = lock_versions();
    set_current_version("2.5");
    let tools = EvolvingTools::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"mul"));
    assert!(!names.contains(&"add"));
}

#[test]
fn test_no_current_version_serves_all_tools() {
    let _lock = lock_versions();
    // Backwards-compatible default: when no version is set, every
    // tool is served (the macro's fast path returns the full
    // TOOLS slice).
    clear_current_version();
    let tools = EvolvingTools::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"mul"));
}

#[test]
fn test_calls_to_active_tool_still_work() {
    let _lock = lock_versions();
    set_current_version("2.5");
    let inst = EvolvingTools::default();
    let result =
        <EvolvingTools as ToolCaller>::call_tool(&inst, "mul", &json!({"a": 3, "b": 4})).unwrap();
    assert_eq!(result, json!(12));
}

#[test]
fn test_filtered_tool_not_in_static_slice_but_still_callable() {
    let _lock = lock_versions();
    // At v1.5 the `mul` method is filtered out of the static
    // `tool_definitions()` slice (so the LLM does not see it
    // as an option), but the underlying Rust method is still
    // reachable through the dispatcher's `call_tool` match.
    // This is by design: T-020 controls the schema surface
    // exposed to the model, not the executable surface.
    set_current_version("1.5");
    let inst = EvolvingTools::default();
    // The schema surface excludes `mul`.
    let tools = EvolvingTools::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(!names.contains(&"mul"), "mul must be hidden at v1.5");
    // Direct invocation through the dispatcher still works
    // because the Rust method is intact.
    let result =
        <EvolvingTools as ToolCaller>::call_tool(&inst, "mul", &json!({"a": 3, "b": 4})).unwrap();
    assert_eq!(result, json!(12));
}

// ============================================================================
// Test impl 2: additive evolution (new optional field in v2).
//
// The 1.0 method has only `a`; the 2.0 method has `a` and an
// optional `b`. The new field is additive — old clients at v1.0
// still work with the 1.0 method.
// ============================================================================

#[derive(Default)]
struct AdditiveEvolution;

#[tool]
impl AdditiveEvolution {
    /// v1: single integer parameter.
    #[tool(since = "1.0", until = "2.0")]
    pub fn query_v1(&self, a: i64) -> i64 {
        a
    }

    /// v2: `a` plus an optional `b`.
    #[tool(since = "2.0")]
    pub fn query_v2(&self, a: i64, b: Option<i64>) -> i64 {
        a + b.unwrap_or(0)
    }
}

#[test]
fn test_additive_change_is_non_breaking() {
    let _lock = lock_versions();
    set_current_version("1.0");
    let tools = AdditiveEvolution::tool_definitions();
    let v1 = tools.iter().find(|t| t.name == "query_v1").unwrap();
    assert!(v1.since.is_some() && v1.until.is_some());

    // The 1.0 schema does NOT carry the new optional field.
    let schema: serde_json::Value = serde_json::from_str(&v1.input_schema).unwrap();
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("a"));
    // `b` is not in the 1.0 schema because the method does not
    // declare it.
    assert!(!props.contains_key("b"));
}

#[test]
fn test_call_to_v1_method_at_v1_works() {
    let _lock = lock_versions();
    set_current_version("1.0");
    let inst = AdditiveEvolution::default();
    let result =
        <AdditiveEvolution as ToolCaller>::call_tool(&inst, "query_v1", &json!({"a": 7})).unwrap();
    assert_eq!(result, json!(7));
}

// ============================================================================
// Test impl 3: version_policy = "semver". The interval check is
// strictly ordered; since >= until produces a compile error.
// ============================================================================

#[derive(Default)]
struct StrictSemVerTools;

#[tool(version_policy = "semver")]
impl StrictSemVerTools {
    /// Available since 1.0.0 (exclusive of 2.0.0).
    #[tool(since = "1.0.0", until = "2.0.0")]
    pub fn v1_tool(&self) -> i64 {
        1
    }

    /// Available since 2.0.0.
    #[tool(since = "2.0.0")]
    pub fn v2_tool(&self) -> i64 {
        2
    }
}

#[test]
fn test_semver_ordered_interval_check() {
    let _lock = lock_versions();
    // 1.10.0 is strictly between 1.0.0 and 2.0.0.
    set_current_version("1.10.0");
    let tools = StrictSemVerTools::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"v1_tool"));
    assert!(!names.contains(&"v2_tool"));
}

#[test]
fn test_semver_equal_boundary_excludes_until_bound() {
    let _lock = lock_versions();
    set_current_version("2.0.0");
    let tools = StrictSemVerTools::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    // The half-open interval [1.0.0, 2.0.0) does NOT include 2.0.0.
    assert!(!names.contains(&"v1_tool"));
    // The new method starts AT 2.0.0, so it IS served.
    assert!(names.contains(&"v2_tool"));
}

// ============================================================================
// Test impl 4: loose strings (CalVer, commit SHA). Accepted without
// `version_policy`; the typed-diff is disabled and runtime uses
// lexicographic ordering.
// ============================================================================

#[derive(Default)]
struct LooseVersioningTools;

#[tool]
impl LooseVersioningTools {
    /// CalVer YYYY.MM.
    #[tool(since = "2026.06")]
    pub fn june_release(&self) -> i64 {
        6
    }

    /// Commit SHA (no version_policy opted in).
    #[tool(since = "abc1234")]
    pub fn dev_build(&self) -> i64 {
        42
    }
}

#[test]
fn test_loose_calver_strings_accepted() {
    let _lock = lock_versions();
    clear_current_version();
    let tools = LooseVersioningTools::tool_definitions();
    let june = tools.iter().find(|t| t.name == "june_release").unwrap();
    assert_eq!(june.since.as_deref(), Some("2026.06"));
    let dev = tools.iter().find(|t| t.name == "dev_build").unwrap();
    assert_eq!(dev.since.as_deref(), Some("abc1234"));
}

#[test]
fn test_loose_strings_lexicographic_dispatch() {
    let _lock = lock_versions();
    // CalVer strings compare lexicographically at runtime when
    // no policy was opted in. "2026.06" < "2026.07" < "2026.07.01"
    // is the natural CalVer ordering; the runtime enforces it.
    set_current_version("2026.06");
    let tools = LooseVersioningTools::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"june_release"));
    assert!(!names.contains(&"dev_build"));
}

// ============================================================================
// Test impl 5: tool without any since/until attribute is always served,
// even when a current version is set. Backwards-compatible default.
// ============================================================================

#[derive(Default)]
struct UnversionedAlongsideVersioned;

#[tool]
impl UnversionedAlongsideVersioned {
    /// A versioned method.
    #[tool(since = "1.0", until = "2.0")]
    pub fn versioned(&self) -> i64 {
        1
    }

    /// A timeless method.
    pub fn always(&self) -> i64 {
        99
    }
}

#[test]
fn test_unversioned_method_always_served() {
    let _lock = lock_versions();
    set_current_version("1.5");
    let tools = UnversionedAlongsideVersioned::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"always"));
    assert!(names.contains(&"versioned"));
}

// ============================================================================
// T-020 trybuild-driven compile-error tests.
//
// Each fixture lives under `tests/ui/` and is paired with a
// `.stderr` snapshot. Refresh the snapshot after a deliberate
// diagnostic change with:
//
//     TRYBUILD=overwrite \
//       cargo test -p tokitai --test schema_evolution_test test_*
// ============================================================================

#[cfg(feature = "serde")]
#[test]
fn test_version_interval_empty_compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/version_interval_empty.rs");
}

#[cfg(feature = "serde")]
#[test]
fn test_version_interval_invalid_semver_compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/version_interval_invalid_semver.rs");
}
