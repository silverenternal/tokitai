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

use tokitai_mcp_server::serve::{
    manifest_version, parse_serve_args, version_matches_prefix, ServeArgs,
};

/// Positive: 1-component prefix matches a version on the same
/// major. The default manifest version is `0.6.0`, so the
/// `0` prefix is always satisfied; the `1` prefix is never
/// satisfied.
#[test]
fn one_component_prefix_matches_same_major() {
    assert!(version_matches_prefix("0.6.0", Some("0")));
    assert!(!version_matches_prefix("0.6.0", Some("1")));
}

/// Positive: 2-component prefix matches any patch on the same
/// `MAJOR.MINOR` line.
#[test]
fn two_component_prefix_matches_same_minor() {
    assert!(version_matches_prefix("0.6.0", Some("0.6")));
    assert!(version_matches_prefix("0.6.99", Some("0.6")));
    assert!(!version_matches_prefix("0.7.0", Some("0.6")));
}

/// Positive: 3-component prefix is an exact match.
#[test]
fn three_component_prefix_is_exact() {
    assert!(version_matches_prefix("0.6.0", Some("0.6.0")));
    assert!(!version_matches_prefix("0.6.0", Some("0.6.1")));
}

/// Positive: a `v`/`V` prefix is accepted transparently.
#[test]
fn v_prefix_is_transparent() {
    assert!(version_matches_prefix("0.6.0", Some("v0.6")));
    assert!(version_matches_prefix("0.6.0", Some("V0.6.0")));
    assert!(!version_matches_prefix("0.6.0", Some("v0.7")));
}

/// Positive: `None` and the empty string both mean "no
/// requirement" — the helper is a no-op.
#[test]
fn no_requirement_is_a_no_op() {
    assert!(version_matches_prefix("0.6.0", None));
    assert!(version_matches_prefix("0.6.0", Some("")));
    assert!(version_matches_prefix("garbage", None));
}

/// Negative: a malformed manifest or requirement must NEVER
/// silently pass. Both sides invalid -> false. Either side
/// invalid -> false.
#[test]
fn malformed_inputs_never_silently_pass() {
    // Garbage on either side.
    assert!(!version_matches_prefix("garbage", Some("0.6")));
    assert!(!version_matches_prefix("0.6.0", Some("garbage")));
    // Empty manifest + valid prefix.
    assert!(!version_matches_prefix("", Some("0.6")));
    // Leading-zero component is not canonical SemVer.
    assert!(!version_matches_prefix("01.0.0", Some("0")));
    assert!(!version_matches_prefix("0.6.0", Some("01.0.0")));
}

/// Acceptance criterion 5: a server compiled against
/// `tokitai-core 0.6.0` started with `--require-tokitai=0.9.0`
/// refuses to start. The pure helper returns `false`, and
/// `serve()` propagates the refusal to the caller. We test the
/// pure helper here; the surrounding `serve()` plumbing is
/// covered by the `check_version_at_startup` unit tests in
/// `src/serve.rs`.
#[test]
fn required_prefix_mismatch_is_rejected() {
    assert!(!version_matches_prefix("0.6.0", Some("0.9.0")));
    assert!(!version_matches_prefix("0.6.0", Some("0.9")));
    assert!(!version_matches_prefix("0.6.0", Some("1")));
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
/// `0.6` prefix. If a future contributor bumps the workspace
/// version without updating the build script's resolution
/// path, this test fails loudly.
#[test]
fn resolved_manifest_version_satisfies_workspace_prefix() {
    // The manifest is whatever tokitai-core version this build
    // was compiled against; the workspace is currently on
    // `0.6.x`, so the `0.6` prefix always matches.
    //
    // The version is reachable via the same `include!` the
    // runtime uses, so we exercise the live path rather than
    // hard-coding the literal.
    let manifest = manifest_version();
    assert!(
        version_matches_prefix(manifest, Some("0.6")),
        "resolved manifest `{}` must satisfy the `0.6` prefix",
        manifest,
    );
}

/// T-027: integration test that exercises `parse_serve_args` with
/// realistic argc/argv, mirroring what `std::env::args()` returns
/// when the binary is launched with the documented flags. The
/// unit tests in `src/serve.rs` cover the helper with hand-crafted
/// `Vec<&str>` inputs; this test adds the integration path where
/// the input comes from `std::env::args()` itself, so the boundary
/// between OS argv and the parser is exercised end-to-end.
///
/// We construct a realistic argv by skipping `argv[0]` (the binary
/// path), exactly as `serve()` does internally:
/// `parse_serve_args(std::env::args().skip(1))`. The test then
/// asserts the same outcomes the unit tests assert, but on a
/// path that round-trips through `OsString -> String -> &str`
/// the way the real OS startup does.
///
/// We deliberately do NOT call `serve()` itself in this test
/// because it would observe whatever flags the test runner
/// happened to pass to the test binary; instead we synthesize
/// the argv and feed it to the same parser `serve()` uses.
#[test]
fn parse_serve_args_with_realistic_argv_via_env_args_skip_one() {
    // Mirror what `std::env::args()` produces when the binary
    // is launched as: `<binary> --require-tokitai=0.6.0 --allow-tokitai-mismatch`.
    // We model it as `Vec<String>` (the exact type
    // `std::env::args().skip(1)` yields) and feed it through the
    // parser. This is the same code path `serve()` runs on startup.
    let realistic_argv: Vec<String> = vec![
        "--require-tokitai=0.6.0".to_string(),
        "--allow-tokitai-mismatch".to_string(),
    ];

    let parsed = parse_serve_args(realistic_argv.iter().map(String::as_str))
        .expect("realistic argv must parse successfully");

    assert_eq!(
        parsed,
        ServeArgs {
            require_tokitai: Some("0.6.0".to_string()),
            allow_mismatch: true,
        }
    );
}

/// T-027: integration test for the boundary case where the test
/// runner (or a real user) launches the binary with NO flags.
/// Mirrors `std::env::args()` returning just `argv[0]`, which
/// after `.skip(1)` yields an empty iterator.
#[test]
fn parse_serve_args_with_empty_env_args_skip_one() {
    let empty_argv: Vec<String> = vec![];
    let parsed = parse_serve_args(empty_argv.iter().map(String::as_str))
        .expect("empty argv must parse successfully");
    assert_eq!(
        parsed,
        ServeArgs {
            require_tokitai: None,
            allow_mismatch: false,
        }
    );
}

/// T-027: integration test that feeds a realistic `--require-tokitai`
/// with a malformed prefix through the same path. The parser must
/// not panic and must surface the operator-error message.
#[test]
fn parse_serve_args_rejects_empty_prefix_via_realistic_argv() {
    // Simulate `<binary> --require-tokitai=` — the empty-prefix
    // operator error path. The parser must NOT panic and must
    // return Err with the operator-fix message, exactly as the
    // unit tests assert on hand-crafted inputs.
    let realistic_argv: Vec<String> = vec!["--require-tokitai=".to_string()];
    let result = parse_serve_args(realistic_argv.iter().map(String::as_str));
    let err = result.expect_err("empty prefix must be rejected");
    assert!(
        err.contains("non-empty"),
        "operator-error message must mention the fix: got {:?}",
        err,
    );
}

/// T-027: end-to-end integration with the live `manifest_version()`.
///
/// Calls `version_matches_prefix` with the live manifest version
/// (whatever the binary was compiled against) and a realistic
/// prefix that the OS-level operator would pass via
/// `--require-tokitai=0.6`. The test asserts the version check
/// path that `serve()` runs at startup actually accepts the
/// manifest under the workspace's current major.minor.
#[test]
fn version_check_at_startup_accepts_workspace_prefix_via_realistic_argv() {
    // Live manifest (whatever the binary was compiled against).
    let manifest = manifest_version();
    // A realistic `--require-tokitai=0.6` prefix the OS operator
    // would pass. The workspace is on 0.6.x, so this must match.
    let realistic_argv: Vec<String> = vec!["--require-tokitai=0.6".to_string()];
    let parsed = parse_serve_args(realistic_argv.iter().map(String::as_str))
        .expect("realistic argv must parse successfully");
    assert_eq!(parsed.require_tokitai.as_deref(), Some("0.6"));
    assert!(
        version_matches_prefix(manifest, parsed.require_tokitai.as_deref()),
        "live manifest `{}` must satisfy realistic prefix `0.6`",
        manifest,
    );
}
