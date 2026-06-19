//! T-024: integration tests for the runtime cross-crate version
//! assertion in `tokitai-mcp-server`.
//!
//! The server-side check fires inside `serve()` (and the
//! `parse_serve_args` / `check_version_at_startup` helpers
//! underneath it). The tests in this file cover the helper
//! surface — the `serve()` top-level entry point is exercised in
//! the `desc_safety_server_test.rs` style tests via the
//! `parse_serve_args` helper, which is the only part of the
//! function that does not bind a port.
//!
//! Acceptance criterion 5 (from `todo.json` v3.0 T-024) calls for
//! a server compiled against tokitai 0.8.1, started with
//! `--require-tokitai=0.9.0`, to log a `warn!` and exit 78
//! (`EX_CONFIG`). The pure `version_matches_prefix` helper
//! covered here is the building block; the surrounding wiring
//! is the responsibility of the `serve()` entry point and is
//! covered by the unit tests in `src/serve.rs`.

use tokitai_mcp_server::serve::{manifest_version, version_matches_prefix};

/// Positive: 1-component prefix matches a version on the same
/// major. The default manifest version is `0.5.1`, so the
/// `0` prefix is always satisfied; the `1` prefix is never
/// satisfied.
#[test]
fn one_component_prefix_matches_same_major() {
    assert!(version_matches_prefix("0.5.1", Some("0")));
    assert!(!version_matches_prefix("0.5.1", Some("1")));
}

/// Positive: 2-component prefix matches any patch on the same
/// `MAJOR.MINOR` line.
#[test]
fn two_component_prefix_matches_same_minor() {
    assert!(version_matches_prefix("0.5.1", Some("0.5")));
    assert!(version_matches_prefix("0.5.99", Some("0.5")));
    assert!(!version_matches_prefix("0.6.0", Some("0.5")));
}

/// Positive: 3-component prefix is an exact match.
#[test]
fn three_component_prefix_is_exact() {
    assert!(version_matches_prefix("0.5.1", Some("0.5.1")));
    assert!(!version_matches_prefix("0.5.1", Some("0.5.2")));
}

/// Positive: a `v`/`V` prefix is accepted transparently.
#[test]
fn v_prefix_is_transparent() {
    assert!(version_matches_prefix("0.5.1", Some("v0.5")));
    assert!(version_matches_prefix("0.5.1", Some("V0.5.1")));
    assert!(!version_matches_prefix("0.5.1", Some("v0.6")));
}

/// Positive: `None` and the empty string both mean "no
/// requirement" — the helper is a no-op.
#[test]
fn no_requirement_is_a_no_op() {
    assert!(version_matches_prefix("0.5.1", None));
    assert!(version_matches_prefix("0.5.1", Some("")));
    assert!(version_matches_prefix("garbage", None));
}

/// Negative: a malformed manifest or requirement must NEVER
/// silently pass. Both sides invalid -> false. Either side
/// invalid -> false.
#[test]
fn malformed_inputs_never_silently_pass() {
    // Garbage on either side.
    assert!(!version_matches_prefix("garbage", Some("0.5")));
    assert!(!version_matches_prefix("0.5.1", Some("garbage")));
    // Empty manifest + valid prefix.
    assert!(!version_matches_prefix("", Some("0.5")));
    // Leading-zero component is not canonical SemVer.
    assert!(!version_matches_prefix("01.0.0", Some("0")));
    assert!(!version_matches_prefix("0.5.1", Some("01.0.0")));
}

/// Acceptance criterion 5: a server compiled against
/// `tokitai-core 0.5.1` started with `--require-tokitai=0.9.0`
/// refuses to start. The pure helper returns `false`, and
/// `serve()` propagates the refusal to the caller. We test the
/// pure helper here; the surrounding `serve()` plumbing is
/// covered by the `check_version_at_startup` unit tests in
/// `src/serve.rs`.
#[test]
fn required_prefix_mismatch_is_rejected() {
    assert!(!version_matches_prefix("0.5.1", Some("0.9.0")));
    assert!(!version_matches_prefix("0.5.1", Some("0.9")));
    assert!(!version_matches_prefix("0.5.1", Some("1")));
}

/// Verify that the workspace root resolution (used by `build.rs`
/// to locate `Cargo.lock`) actually works from the test crate's
/// own `CARGO_MANIFEST_DIR`.  If the workspace layout changes
/// (e.g. a crate moves under `crates/foo/`), this test breaks
/// before the build script silently produces an unresolved
/// manifest.
#[test]
fn workspace_root_contains_cargo_lock_with_tokitai_core() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = false;
    let mut dir = Some(manifest_dir);
    while let Some(d) = dir {
        let toml = d.join("Cargo.toml");
        if let Ok(content) = std::fs::read_to_string(&toml) {
            if content.lines().any(|l| l.trim() == "[workspace]") {
                let lock = d.join("Cargo.lock");
                let lock_content =
                    std::fs::read_to_string(&lock).expect("Cargo.lock at workspace root");
                assert!(
                    lock_content.contains("\"tokitai-core\""),
                    "Cargo.lock at {:?} must contain a [[package]] entry for tokitai-core",
                    lock
                );
                found = true;
                break;
            }
        }
        dir = d.parent();
    }
    assert!(found, "workspace root not found from {:?}", manifest_dir);
}
/// baked into the test binary by `build.rs`, satisfies its own
/// `0.5` prefix. If a future contributor bumps the workspace
/// version without updating the build script's resolution
/// path, this test fails loudly.
#[test]
fn resolved_manifest_version_satisfies_workspace_prefix() {
    // The manifest is whatever tokitai-core version this build
    // was compiled against; the workspace is currently on
    // `0.5.x`, so the `0.5` prefix always matches.
    //
    // The version is reachable via the same `include!` the
    // runtime uses, so we exercise the live path rather than
    // hard-coding the literal.
    let manifest = manifest_version();
    assert!(
        version_matches_prefix(manifest, Some("0.5")),
        "resolved manifest `{}` must satisfy the `0.5` prefix",
        manifest,
    );
}
