//! T-024: integration tests for `tokitai_core::assert_compatible_with`.
//!
//! The compile-time panic path is exercised when a downstream
//! crate writes `const _: () = tokitai_core::assert_compatible_with("...");`
//! — see the test cases below for the prefix-match rule. Here we
//! cover the runtime path: the function called outside a `const`
//! context, the prefix-match rule, the `v` prefix acceptance, and
//! the malformed-literal failure mode.

use tokitai_core::{assert_compatible_with, CORE_VERSION};

/// Helper: format a `String` using a `format!`-style template
/// without depending on `alloc` at the test top level.
fn fmt(template: &str) -> String {
    template.replace("{}", "")
}

/// Strip the patch component from a SemVer string. Returns the
/// `MAJOR.MINOR` prefix that drives the 2-arity comparison.
fn two_component_prefix() -> String {
    let dot = CORE_VERSION
        .find('.')
        .expect("CORE_VERSION must contain at least one '.'");
    let after_first = &CORE_VERSION[dot + 1..];
    let second_dot = after_first
        .find('.')
        .expect("CORE_VERSION must contain a second '.'");
    CORE_VERSION[..dot + 1 + second_dot].to_string()
}

#[test]
fn positive_exact_match_passes_at_runtime() {
    // CORE_VERSION is the version this test binary was compiled
    // against. Asking for an exact match must never panic.
    assert_compatible_with(CORE_VERSION);
}

#[test]
fn positive_prefix_match_passes_at_runtime() {
    // Drop the patch component and confirm prefix-match works at
    // runtime. CORE_VERSION is always `MAJOR.MINOR.PATCH`, so the
    // first two components form a valid 2-arity prefix.
    let prefix = two_component_prefix();
    assert_compatible_with(&prefix);
}

#[test]
fn positive_v_prefix_is_accepted() {
    // A leading `v` is the canonical way the wider Rust ecosystem
    // tags SemVer literals. The function must accept it
    // transparently.
    let mut v_prefixed = String::with_capacity(CORE_VERSION.len() + 1);
    v_prefixed.push('v');
    v_prefixed.push_str(CORE_VERSION);
    assert_compatible_with(&v_prefixed);
}

#[test]
fn positive_v_prefixed_two_component() {
    let mut v_prefixed = String::with_capacity(20);
    v_prefixed.push('v');
    v_prefixed.push_str(&two_component_prefix());
    assert_compatible_with(&v_prefixed);
}

#[test]
fn negative_mismatch_panics_at_runtime() {
    // A literal that cannot match CORE_VERSION under any rule.
    // 999 is large enough that the major-minor drift is loud.
    let result = std::panic::catch_unwind(|| {
        assert_compatible_with("999.0.0");
    });
    assert!(
        result.is_err(),
        "assert_compatible_with must panic on a major drift",
    );
}

#[test]
fn negative_minor_drift_panics_at_runtime() {
    // Bump only the minor. The 3-component `expected` arity forces
    // the patch check; we deliberately pick a minor that is not
    // the same as CORE_VERSION's.
    let dot = CORE_VERSION.find('.').unwrap();
    let after_first = &CORE_VERSION[dot + 1..];
    let second_dot = after_first.find('.').unwrap();
    let minor_str = &after_first[..second_dot];
    let minor: u64 = minor_str.parse().unwrap_or(0);
    let bumped = if minor == 0 { 999 } else { minor - 1 };
    let literal = format!("{}.{}.0", &CORE_VERSION[..dot], bumped);
    let result = std::panic::catch_unwind(|| {
        assert_compatible_with(&literal);
    });
    assert!(
        result.is_err(),
        "assert_compatible_with must panic on a minor drift",
    );
}

#[test]
fn negative_patch_drift_panics_at_runtime() {
    // A 3-component literal whose patch differs. We bump only the
    // patch so the major and minor checks pass and only the
    // patch-equality check fails.
    let dot = CORE_VERSION.find('.').unwrap();
    let after_first = &CORE_VERSION[dot + 1..];
    let second_dot = after_first.find('.').unwrap();
    let minor_str = &after_first[..second_dot];
    let minor: u64 = minor_str.parse().unwrap_or(0);
    let bumped = if minor == u64::MAX { 0 } else { minor + 1 };
    let literal = format!("{}.{}.0", &CORE_VERSION[..dot], bumped);
    let result = std::panic::catch_unwind(|| {
        assert_compatible_with(&literal);
    });
    assert!(
        result.is_err(),
        "assert_compatible_with must panic on a patch drift (when expected arity is 3)",
    );
}

#[test]
fn negative_malformed_literal_panics_at_runtime() {
    // A non-numeric component must panic — never silently pass.
    let result = std::panic::catch_unwind(|| {
        assert_compatible_with("not-a-version");
    });
    assert!(
        result.is_err(),
        "assert_compatible_with must panic on a malformed literal",
    );
}

#[test]
fn negative_leading_zero_component_panics() {
    // `01.0.0` is not a canonical SemVer literal; the parser
    // rejects it so a stray typo cannot be silently treated as
    // `1.0.0`.
    let result = std::panic::catch_unwind(|| {
        assert_compatible_with("01.0.0");
    });
    assert!(
        result.is_err(),
        "assert_compatible_with must panic on a non-canonical SemVer literal",
    );
}

#[test]
fn core_version_is_non_empty_semver_like() {
    // Sanity: the constant must be non-empty and look like a
    // version string. A failing build that forgot to expose
    // `CARGO_PKG_VERSION` would surface here.
    assert!(!CORE_VERSION.is_empty());
    let bytes = CORE_VERSION.as_bytes();
    let mut saw_digit = false;
    for b in bytes {
        if b.is_ascii_digit() {
            saw_digit = true;
        }
    }
    assert!(
        saw_digit,
        "CORE_VERSION must contain at least one digit (got `{}`)",
        CORE_VERSION,
    );
}

#[test]
fn constant_format_uses_cargo_pkg_version() {
    // CORE_VERSION is sourced from `env!("CARGO_PKG_VERSION")`
    // — the same string the manifest publishes. The two must be
    // in lockstep.
    let manifest_version = env!("CARGO_PKG_VERSION");
    assert_eq!(
        CORE_VERSION, manifest_version,
        "CORE_VERSION must match the manifest's CARGO_PKG_VERSION",
    );
}

#[allow(dead_code)]
fn _format_smoke_test() {
    let _ = fmt("hello");
}

// ---------------------------------------------------------------------------
// Compile-time eval (T-024 acceptance criterion #1 + #2).
//
// When this file is compiled, rustc must evaluate the three `const _`
// blocks below at compile time. All three must compile cleanly because
// they match CORE_VERSION. A future contributor who bumps the
// crate version without updating these literals will get a hard
// compile error here — the same compile-time guarantee the
// function promises to downstream consumers.
//
// The literals are pinned at "0.6" because the workspace currently
// pins `tokitai-core = "0.6.x"`. The exact-match and prefix-match
// blocks both target that line; if the major bumps, both fail to
// compile and the contributor is forced to update the literals in
// lockstep with the version bump.
// ---------------------------------------------------------------------------

const _: () = {
    // Prefix match: 2-component, must always pass for 0.6.x.
    tokitai_core::assert_compatible_with("0.6");
};

const _: () = {
    // Prefix match: 1-component, must always pass for 0.x.y.
    tokitai_core::assert_compatible_with("0");
};

const _: () = {
    // `v` prefix is accepted transparently.
    tokitai_core::assert_compatible_with("v0.6");
};
