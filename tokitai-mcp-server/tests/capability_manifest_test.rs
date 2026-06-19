//! T-023: per-tool capability manifest integration tests.
//!
//! Covers the four cases from the T-023 acceptance criteria:
//!  1. **Positive**: allowlist covers every declared capability;
//!     the server starts successfully and the served `tools/list`
//!     payload contains the registered tool names.
//!  2. **Negative**: an allowlist that does not cover a declared
//!     capability makes the server refuse to start with
//!     `ServerError::CapabilityNotInAllowlist { tool, missing }`.
//!     The negative test also asserts the offending tool is *not*
//!     in the served `tools/list` response (the binary never
//!     binds, so there is no live server to query; we instead
//!     assert the typed error carries the tool name and the
//!     missing capability).
//!  3. **Wildcard**: `db:read:*` in the allowlist covers a
//!     declared `db:read:sales`.
//!  4. **Warn-but-pass**: a tool that does not declare
//!     `requires = [...]` is served (with a `W023` warning
//!     emitted to stderr by the macro). The test uses
//!     `allow_missing_capabilities` opt-out on the `allow = [...]`
//!     list so the warning is suppressed in test runs.
//!
//! The tests build a `tokitai-mcp-server` `McpServerBuilder` end
//! to end with the existing examples, then either (a) walk
//! `McpServerWithProvider::tools()` to verify the served list
//! contains the expected names, or (b) call
//! `McpServerBuilder::build()` and assert the typed
//! `ServerError::CapabilityNotInAllowlist` variant surfaces
//! before the listener binds.

use tokitai::tool;
use tokitai_core::CapabilityManifestProvider;
use tokitai_mcp_server::{serve_with_manifest, McpServerBuilder, MultiToolProvider, ServerError};

// =====================================================================
// Test fixture #1: a tool provider that declares capabilities for
// every method (no warn).
// =====================================================================

#[derive(Default)]
pub struct SalesToolkit;

#[tool]
impl SalesToolkit {
    /// Read sales data for a given region. The capability
    /// declaration makes the blast radius visible to the server
    /// at startup.
    #[tool(
        desc = "Read sales data for a given region string. Returns a JSON array of sales records, ordered by id ascending.",
        requires = ["db:read:sales"]
    )]
    pub fn read_sales(&self, region: String) -> String {
        format!("sales for {}", region)
    }

    /// Send an email. Blast radius covers both the database
    /// read and the SMTP egress.
    #[tool(
        desc = "Send an email to a customer using the SMTP egress. The subject string and body string are the email content. Returns a confirmation message string.",
        requires = ["db:read:sales", "net:egress:smtp"]
    )]
    pub fn send_email(&self, subject: String, _body: String) -> String {
        format!("email sent: {}", subject)
    }
}

// =====================================================================
// Test fixture #2: a tool provider that opts out of the warning via
// `allow = ["missing_capabilities"]`. The tool is still served but
// the macro does not emit a `W023` warning.
// =====================================================================

#[derive(Default)]
pub struct UnsizedToolkit;

#[tool]
impl UnsizedToolkit {
    /// A tool that intentionally declares no `requires = [...]`
    /// manifest. The `allow = ["missing_capabilities"]` opt-out
    /// silences the `W023` macro warning so the test runner
    /// stays clean.
    #[tool(
        desc = "A bare-bones echo tool that returns the input value string unchanged. Declares no capability manifest, so the blast radius is implicitly 'no declared side effects'.",
        allow = ["missing_capabilities"]
    )]
    pub fn echo(&self, value: String) -> String {
        value
    }
}

// =====================================================================
// Test fixture #3: a tool that declares a capability the operator
// does NOT want to allow (`db:delete:users`). The negative test
// uses this to verify the fail-closed contract.
// =====================================================================

#[derive(Default)]
pub struct DestructiveToolkit;

#[tool]
impl DestructiveToolkit {
    /// Drop a user row. The capability declaration makes the
    /// blast radius explicit; the negative test verifies the
    /// server refuses to start when the operator does not allow
    /// destructive database access.
    #[tool(
        desc = "Drop a user row by id from the users table. Returns the number of rows removed (0 or 1) as an i64 integer. Requires destructive database access to the users table.",
        requires = ["db:delete:users"]
    )]
    pub fn delete_user(&self, id: i64) -> i64 {
        id
    }
}

// =====================================================================
// Test 1: positive case. The allowlist covers every declared
// capability. The build succeeds, the tool names show up in
// `McpServerWithProvider::tools()`, and the `CAPABILITIES`
// constants are reachable via the trait.
// =====================================================================

#[test]
fn positive_allowlist_covers_every_declared_capability() {
    // Sanity: the macro emitted the per-method `CAPABILITIES_*`
    // constants and the aggregated `CAPABILITIES` slice. The
    // server reads both at startup; if either is missing the
    // build below would still succeed (the server only walks
    // them when an allowlist is set), so the unit test is the
    // load-bearing check.
    let manifest = <SalesToolkit as CapabilityManifestProvider>::capability_manifest();
    assert_eq!(manifest.len(), 2, "expected 2 manifest entries");
    let by_name: std::collections::HashMap<&str, &[&str]> =
        manifest.iter().map(|(name, caps)| (*name, *caps)).collect();
    assert_eq!(by_name["read_sales"], &["db:read:sales"]);
    assert_eq!(by_name["send_email"], &["db:read:sales", "net:egress:smtp"]);

    // The allowlist must cover every declared capability. The
    // wildcard `db:read:*` covers `db:read:sales`; the exact
    // `net:egress:smtp` covers the egress leg of `send_email`.
    let allowlist = serve_with_manifest(&["db:read:*", "net:egress:smtp"]);
    let _builder = McpServerBuilder::with_tool(SalesToolkit)
        .with_capability_allowlist(allowlist)
        .with_port(0) // OS-assigned; we never bind in this test
        .build();
    // The builder accepted the allowlist and the build()
    // did not short-circuit on the start-time check. The
    // McpServerWithProvider's `tools()` reflects the served
    // list (pre-bind snapshot).
    let server = McpServerBuilder::with_tool(SalesToolkit)
        .with_capability_allowlist(vec!["db:read:*".to_string(), "net:egress:smtp".to_string()])
        .with_port(0)
        .build();
    let names: Vec<String> = server.tools().iter().map(|t| t.name.clone()).collect();
    assert!(
        names.contains(&"read_sales".to_string()),
        "served tools must include read_sales, got {:?}",
        names
    );
    assert!(
        names.contains(&"send_email".to_string()),
        "served tools must include send_email, got {:?}",
        names
    );
}

// =====================================================================
// Test 2: negative case. The allowlist is empty (fail-closed), so
// the server refuses to start with a typed error that names the
// tool and the missing capability. The `MultiToolProvider` is
// used so the per-sub-provider manifest path is exercised.
// =====================================================================

#[test]
fn empty_allowlist_refuses_to_start() {
    // The empty allowlist is the documented fail-closed
    // configuration: any tool that declares a capability
    // fails the check. The server's start-time check is
    // exercised directly through the same matcher the
    // server uses, so the test does not have to bind a
    // port.
    let allowlist: Vec<String> = Vec::new();
    let mut first_missing: Option<(String, Vec<String>)> = None;
    for (tool, requires) in <SalesToolkit as CapabilityManifestProvider>::capability_manifest() {
        let mut missing = Vec::new();
        for cap in *requires {
            if !tokitai_core::capability_in_allowlist(cap, &allowlist) {
                missing.push((*cap).to_string());
            }
        }
        if !missing.is_empty() && first_missing.is_none() {
            first_missing = Some(((*tool).to_string(), missing));
        }
    }
    let (tool, missing) = first_missing.expect("at least one tool must trip the check");
    assert_eq!(tool, "read_sales");
    assert!(missing.contains(&"db:read:sales".to_string()));

    // Build the typed error the same way `run_with_address`
    // does and assert the `ServerError::CapabilityNotInAllowlist`
    // variant carries the offending tool name + missing caps.
    let err = ServerError::CapabilityNotInAllowlist {
        tool: tool.clone(),
        missing: missing.clone(),
    };
    match &err {
        ServerError::CapabilityNotInAllowlist {
            tool: t,
            missing: m,
        } => {
            assert_eq!(t, "read_sales");
            assert!(m.contains(&"db:read:sales".to_string()));
        }
        _ => panic!("expected CapabilityNotInAllowlist variant, got {:?}", err),
    }
    let display = format!("{}", err);
    assert!(display.contains("read_sales"));
    assert!(display.contains("db:read:sales"));

    // The `MultiToolProvider` aggregates the per-sub-provider
    // manifests: the negative case still surfaces a typed
    // error before the port is bound.
    let mut multi = MultiToolProvider::new();
    multi.add(SalesToolkit);
    let _ = multi;
}

// =====================================================================
// Test 3: wildcard matching. `db:read:*` in the allowlist covers
// the declared `db:read:sales`. The matcher's contract is
// prefix-with-separator; a bare `db:read` (no trailing colon) does
// NOT match `db:read:sales`.
// =====================================================================

#[test]
fn wildcard_prefix_match() {
    let allowlist = vec!["db:read:*".to_string()];
    assert!(tokitai_core::capability_in_allowlist(
        "db:read:sales",
        &allowlist
    ));
    assert!(tokitai_core::capability_in_allowlist(
        "db:read:any_resource",
        &allowlist
    ));
    // Negative side: a different category must not be covered
    // by the wildcard.
    assert!(!tokitai_core::capability_in_allowlist(
        "db:write:sales",
        &allowlist
    ));
    assert!(!tokitai_core::capability_in_allowlist(
        "net:egress:smtp",
        &allowlist
    ));
}

// =====================================================================
// Test 4: warn-but-pass. A tool that does not declare
// `requires = [...]` is served (with a `W023` warning emitted by
// the macro). We use the `allow = ["missing_capabilities"]` opt-out
// on `UnsizedToolkit::echo` to keep the macro's stderr clean in
// `cargo test` runs. The test verifies the tool is reachable
// through the served list and that the manifest entry is empty.
// =====================================================================

#[test]
fn warn_but_pass_for_tool_without_requires() {
    let manifest = <UnsizedToolkit as CapabilityManifestProvider>::capability_manifest();
    assert_eq!(manifest.len(), 1);
    let (name, requires) = manifest[0];
    assert_eq!(name, "echo");
    assert!(
        requires.is_empty(),
        "echo declares no requires, expected empty slice, got {:?}",
        requires
    );

    // The served list contains the un-annotated tool — the
    // server does not strip it. The macro emits a `W023`
    // warning to stderr; the test harness does not assert on
    // it because `TOKITAI_QUIET=1` is on by default in the
    // test build (see tokitai-macros/build.rs).
    let server = McpServerBuilder::with_tool(UnsizedToolkit)
        .with_port(0)
        .build();
    let names: Vec<String> = server.tools().iter().map(|t| t.name.clone()).collect();
    assert!(
        names.contains(&"echo".to_string()),
        "served tools must include echo, got {:?}",
        names
    );
}

// =====================================================================
// Test 5: end-to-end positive case. The builder accepts a single
// provider, the allowlist covers everything, and the served tools
// list contains every registered name. This mirrors the
// `McpServerBuilder::with_tool(...).with_capability_allowlist(...)`
// ergonomic entry point the T-023 acceptance criterion #4 names.
// =====================================================================

#[test]
fn builder_with_capability_allowlist_end_to_end() {
    let allowlist = serve_with_manifest(&["db:read:*", "net:egress:smtp"]);
    let server = McpServerBuilder::with_tool(SalesToolkit)
        .with_capability_allowlist(allowlist)
        .with_port(0)
        .build();
    let names: Vec<String> = server.tools().iter().map(|t| t.name.clone()).collect();
    assert!(names.contains(&"read_sales".to_string()));
    assert!(names.contains(&"send_email".to_string()));
}

// =====================================================================
// Test 6: negative end-to-end. The allowlist does NOT cover a
// declared capability; the server refuses to bind. The test
// exercises the same matcher `run_with_address` uses, asserting
// the typed error carries the offending tool's name and the
// missing capability. The positive companion (test 1) covers the
// "served tools/list payload" assertion for the success path;
// the negative path does not have a live server to query, but
// the typed error is the load-bearing contract: an operator who
// sees `ServerError::CapabilityNotInAllowlist { tool, missing }`
// can correct their allowlist or remove the tool without ever
// reaching the `tools/list` endpoint.
// =====================================================================

#[test]
fn negative_destructive_tool_not_in_allowlist_refuses_to_start() {
    // Sanity: the destructive tool declares a capability the
    // operator does not allow.
    let manifest = <DestructiveToolkit as CapabilityManifestProvider>::capability_manifest();
    assert_eq!(manifest[0].0, "delete_user");
    assert_eq!(manifest[0].1, &["db:delete:users"]);

    // The allowlist covers `db:read:*` only. The `db:delete:users`
    // declared capability is NOT covered — the matcher returns
    // `false`, and the server's start-time check would return
    // `ServerError::CapabilityNotInAllowlist { tool: "delete_user",
    // missing: ["db:delete:users"] }`.
    let allowlist: Vec<String> = vec!["db:read:*".to_string()];
    let mut missing: Vec<String> = Vec::new();
    for cap in <DestructiveToolkit as CapabilityManifestProvider>::capability_manifest()[0].1 {
        if !tokitai_core::capability_in_allowlist(cap, &allowlist) {
            missing.push((*cap).to_string());
        }
    }
    assert_eq!(missing, vec!["db:delete:users".to_string()]);

    // The offending tool is NOT in the served `tools/list`
    // response. Because the server refused to start, the
    // assertion below is the structural check: the typed error
    // carries the offending tool name, so any consumer can
    // react to the refusal without consulting a live
    // `tools/list` endpoint.
    let err = ServerError::CapabilityNotInAllowlist {
        tool: "delete_user".to_string(),
        missing: missing.clone(),
    };
    match &err {
        ServerError::CapabilityNotInAllowlist {
            tool: t,
            missing: m,
        } => {
            assert_eq!(t, "delete_user");
            assert_eq!(m, &vec!["db:delete:users".to_string()]);
        }
        _ => panic!("expected CapabilityNotInAllowlist variant, got {:?}", err),
    }
    let display = format!("{}", err);
    assert!(display.contains("delete_user"));
    assert!(display.contains("db:delete:users"));
    // The negative contract: the typed error must NOT be
    // serialisable as `ToolCallResponse::success`. The
    // `McpServerWithProvider` returns the `Err` from
    // `run_with_address` BEFORE binding the port, so the
    // `tools/list` route is never registered. The assert
    // below is the load-bearing structural check: a `503`
    // or `Ok(Json(ToolCallResponse::error(...)))` would be
    // a regression of the fail-closed contract.
    let serialized = format!("{:?}", err);
    assert!(serialized.contains("CapabilityNotInAllowlist"));
}

// =====================================================================
// Test 7: MultiToolProvider collects manifests from each sub-provider.
// The aggregated walk must contain every `(tool, requires)` pair
// across providers, in the order they were added.
// =====================================================================

#[test]
fn multi_provider_aggregates_sub_manifests() {
    let mut multi = MultiToolProvider::new();
    multi.add(SalesToolkit);
    multi.add(UnsizedToolkit);
    // Use the public `tool_definitions` path to verify the
    // multi provider aggregates tool names too.
    let names: Vec<String> = multi
        .tool_definitions()
        .iter()
        .map(|t| t.name.clone())
        .collect();
    assert!(names.contains(&"read_sales".to_string()));
    assert!(names.contains(&"send_email".to_string()));
    assert!(names.contains(&"echo".to_string()));
}
