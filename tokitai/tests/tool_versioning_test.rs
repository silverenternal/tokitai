//! T-013: end-to-end test for the version / `deprecated_since` /
//! `remove_in` / `replaced_by` lifecycle on `#[tool]`-generated
//! providers. The acceptance criteria are:
//!
//! 1. `#[tool(version = "2.1.0")]` and the deprecation triple
//!    `#[tool(deprecated_since = "...", remove_in = "...", replaced_by = "...")]`
//!    flow through the macro into the emitted `ToolDefinition`.
//! 2. Generated `__call_*` wrappers refuse to call a tool whose
//!    `remove_in` is at or before the program's current version
//!    (set via `tokitai::set_current_version`); the returned error
//!    has kind `ToolErrorKind::Removed` and a structured message
//!    that points at the `replaced_by` successor.
//! 3. `to_openai_function` / `to_anthropic_tool` / `to_mcp_tool`
//!    surface the deprecation metadata on their envelopes. The MCP
//!    envelope in particular gains a `_meta` object with
//!    `deprecated`, `deprecatedSince`, `removeIn`, and `replacedBy`.
//! 4. Calls to non-existent tools are routed through the
//!    `replaced_by` table when one is configured at the dispatcher
//!    level.
//!
//! Run with: `cargo test -p tokitai --test tool_versioning_test`

#![cfg(feature = "serde")]
#![allow(non_snake_case, non_upper_case_globals)]
#![allow(clippy::default_constructed_unit_structs, clippy::useless_format)]

use serde_json::json;
use tokitai::tool;
use tokitai::{set_current_version, ToolCaller, ToolErrorKind, ToolProvider};

// ============================================================================
// Test impl 1: a stable tool, a deprecated tool, and a removed tool.
// ============================================================================

#[derive(Default)]
struct VersionedTools;

#[tool]
impl VersionedTools {
    /// Add two numbers.
    #[tool(version = "2.1.0")]
    pub fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }

    /// Multiply two numbers (legacy implementation).
    #[tool(
        version = "1.0.0",
        deprecated = true,
        deprecated_since = "1.5.0",
        remove_in = "2.0.0",
        replaced_by = "add"
    )]
    pub fn legacy_multiply(&self, a: i64, b: i64) -> i64 {
        a * b
    }

    /// Sum an array of numbers (newer, replaces `add` semantically for
    /// batch use cases).
    #[tool(version = "2.1.0")]
    pub fn sum(&self, values: Vec<i64>) -> i64 {
        values.iter().sum()
    }
}

// ============================================================================
// Acceptance criterion 1: macro parses the attributes and they land on
// the emitted `ToolDefinition`.
// ============================================================================

#[test]
fn test_version_attribute_lands_on_definition() {
    let tools = VersionedTools::tool_definitions();

    let add = tools.iter().find(|t| t.name == "add").unwrap();
    assert_eq!(add.version.as_deref(), Some("2.1.0"));
    assert_eq!(add.deprecated_since, None);
    assert_eq!(add.remove_in, None);
    assert_eq!(add.replaced_by, None);

    let legacy = tools.iter().find(|t| t.name == "legacy_multiply").unwrap();
    assert_eq!(legacy.version.as_deref(), Some("1.0.0"));
    assert_eq!(legacy.deprecated_since.as_deref(), Some("1.5.0"));
    assert_eq!(legacy.remove_in.as_deref(), Some("2.0.0"));
    assert_eq!(legacy.replaced_by.as_deref(), Some("add"));
}

#[test]
fn test_deprecation_marker_serialized_into_input_schema() {
    let tools = VersionedTools::tool_definitions();
    let legacy = tools.iter().find(|t| t.name == "legacy_multiply").unwrap();

    // The schema generator embeds the lifecycle fields as
    // `x-deprecated-since` / `x-remove-in` extension members; the
    // presence of these on the stored JSON schema is part of the
    // contract — they are what the MCP / OpenAI / Anthropic
    // envelopes round-trip back to the LLM client.
    let schema: serde_json::Value = serde_json::from_str(&legacy.input_schema).unwrap();
    assert_eq!(schema["x-deprecated-since"], json!("1.5.0"));
    assert_eq!(schema["x-remove-in"], json!("2.0.0"));
}

// ============================================================================
// Acceptance criterion 2: a removed tool is rejected by the
// dispatcher with `ToolErrorKind::Removed`.
// ============================================================================

#[test]
fn test_call_to_removed_tool_returns_removed_error() {
    // Lock the program to a version at or after `remove_in = 2.0.0`.
    set_current_version("2.5.0");
    let tools_inst = VersionedTools::default();
    let _ = VersionedTools::tool_definitions(); // force materialisation

    let result = <VersionedTools as ToolCaller>::call_tool(
        &tools_inst,
        "legacy_multiply",
        &json!({"a": 2, "b": 3}),
    );
    let err = result.expect_err("legacy_multiply is removed in 2.0.0; must be rejected");
    assert_eq!(err.kind, ToolErrorKind::Removed);
    let msg = format!("{}", err.message);
    assert!(
        msg.contains("legacy_multiply"),
        "error message must name the removed tool: {}",
        msg
    );
    assert!(
        msg.contains("2.0.0"),
        "error message must name the remove_in version: {}",
        msg
    );
    assert!(
        msg.contains("add"),
        "error message must point at the replaced_by successor: {}",
        msg
    );
}

#[test]
fn test_call_to_removed_tool_with_equal_current_version_also_rejected() {
    set_current_version("2.0.0");
    let tools_inst = VersionedTools::default();
    let _ = VersionedTools::tool_definitions();
    let result = <VersionedTools as ToolCaller>::call_tool(
        &tools_inst,
        "legacy_multiply",
        &json!({"a": 2, "b": 3}),
    );
    let err = result.expect_err("remove_in is inclusive of the boundary");
    assert_eq!(err.kind, ToolErrorKind::Removed);
}

#[test]
fn test_call_to_current_tool_still_succeeds_after_version_set() {
    set_current_version("2.5.0");
    let tools_inst = VersionedTools::default();
    let result =
        <VersionedTools as ToolCaller>::call_tool(&tools_inst, "add", &json!({"a": 2, "b": 3}))
            .unwrap();
    assert_eq!(result, json!(5));
}

// ============================================================================
// Acceptance criterion 3: provider envelopes surface deprecation
// metadata. MCP gets a structured `_meta` block; OpenAI and Anthropic
// get a description suffix the LLM can read.
// ============================================================================

#[test]
fn test_mcp_envelope_includes_deprecation_meta() {
    let tools = VersionedTools::tool_definitions();
    let legacy = tools.iter().find(|t| t.name == "legacy_multiply").unwrap();
    let mcp = legacy.to_mcp_tool();

    assert!(mcp.get("_meta").is_some(), "MCP envelope must carry _meta");
    let meta = &mcp["_meta"];
    assert_eq!(meta["deprecated"], json!(true));
    assert_eq!(meta["deprecatedSince"], json!("1.5.0"));
    assert_eq!(meta["removeIn"], json!("2.0.0"));
    assert_eq!(meta["replacedBy"], json!("add"));
}

#[test]
fn test_openai_envelope_uses_description_suffix() {
    let tools = VersionedTools::tool_definitions();
    let legacy = tools.iter().find(|t| t.name == "legacy_multiply").unwrap();
    let openai = legacy.to_openai_function();

    let desc = openai["function"]["description"]
        .as_str()
        .expect("OpenAI description must be a string");
    assert!(
        desc.contains("[DEPRECATED"),
        "OpenAI description must include [DEPRECATED ...] suffix: {}",
        desc
    );
    assert!(desc.contains("since=1.5.0"), "got: {}", desc);
    assert!(desc.contains("remove_in=2.0.0"), "got: {}", desc);
    assert!(desc.contains("replaced_by=add"), "got: {}", desc);
}

#[test]
fn test_anthropic_envelope_uses_description_suffix() {
    let tools = VersionedTools::tool_definitions();
    let legacy = tools.iter().find(|t| t.name == "legacy_multiply").unwrap();
    let anthropic = legacy.to_anthropic_tool();

    let desc = anthropic["description"]
        .as_str()
        .expect("Anthropic description must be a string");
    assert!(
        desc.contains("[DEPRECATED"),
        "Anthropic description must include [DEPRECATED ...] suffix: {}",
        desc
    );
    assert!(desc.contains("replaced_by=add"), "got: {}", desc);
}

#[test]
fn test_non_deprecated_tool_envelopes_have_no_meta_or_suffix() {
    let tools = VersionedTools::tool_definitions();
    let add = tools.iter().find(|t| t.name == "add").unwrap();

    let openai = add.to_openai_function();
    let desc = openai["function"]["description"].as_str().unwrap();
    assert!(
        !desc.contains("[DEPRECATED"),
        "stable tool description must not carry deprecation marker: {}",
        desc
    );

    let mcp = add.to_mcp_tool();
    assert!(
        mcp.get("_meta").is_none(),
        "stable tool MCP envelope must not carry _meta"
    );
}

// ============================================================================
// Acceptance criterion 4: a non-existent tool name that matches a
// `replaced_by` source is routed to the successor by the dispatcher.
// ============================================================================

#[test]
fn test_dispatch_to_replaced_by_redirects_to_successor() {
    set_current_version("1.6.0");
    let tools_inst = VersionedTools::default();
    // At v1.6.0 the tool is deprecated but not yet removed; the call
    // must succeed. This is the "warning path" — the call goes
    // through the match arm and reaches the legacy method.
    let result = <VersionedTools as ToolCaller>::call_tool(
        &tools_inst,
        "legacy_multiply",
        &json!({"a": 2, "b": 3}),
    )
    .unwrap();
    assert_eq!(result, json!(6));
}

#[derive(Default)]
struct AliasRedirectTools;

#[tool]
impl AliasRedirectTools {
    /// The successor.
    #[tool(version = "2.0.0")]
    pub fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }

    /// A skipped tool whose `replaced_by` lives in the redirect
    /// table but does NOT generate a match arm (because `skip`).
    /// Calling the old name must therefore route through the
    /// `_ => replaced_by` arm and re-invoke `add` with the args.
    #[tool(skip, replaced_by = "add", version = "1.0.0")]
    pub fn legacy_add(&self, _a: i64, _b: i64) -> i64 {
        0
    }
}

#[test]
fn test_replaced_by_redirects_when_source_is_skipped() {
    let tools_inst = AliasRedirectTools::default();
    // `legacy_add` was removed from the match arms (`#[tool(skip)]`).
    // The dispatcher must fall through to the `replaced_by`
    // redirect and re-dispatch as `add`.
    let result = <AliasRedirectTools as ToolCaller>::call_tool(
        &tools_inst,
        "legacy_add",
        &json!({"a": 4, "b": 6}),
    )
    .unwrap();
    assert_eq!(result, json!(10));
}

#[test]
fn test_dispatcher_does_not_loop_on_missing_replacement() {
    #[derive(Default)]
    struct GhostRedirectTools;

    #[tool]
    impl GhostRedirectTools {
        #[tool(skip, replaced_by = "nonexistent_target", version = "1.0.0")]
        pub fn ghost_source(&self) -> i64 {
            0
        }

        /// The successor (so the trait `ToolCaller` impl has a real
        /// `call_tool` method to route to, instead of having only
        /// the redirect fallback). The redirect to
        /// `nonexistent_target` must not loop because that name is
        /// not in the dispatcher's match arms; the second pass
        /// returns `NotFound`.
        pub fn real_tool(&self) -> i64 {
            42
        }
    }

    let tools_inst = GhostRedirectTools::default();
    let result =
        <GhostRedirectTools as ToolCaller>::call_tool(&tools_inst, "ghost_source", &json!({}));
    let err = result.expect_err("redirect to a non-existent tool must not loop");
    assert_eq!(err.kind, ToolErrorKind::NotFound);
}

// ============================================================================
// Sanity: a tool without any version metadata is not gated by
// `set_current_version`.
// ============================================================================

#[derive(Default)]
struct UnversionedTools;

#[tool]
impl UnversionedTools {
    pub fn ping(&self) -> String {
        "pong".to_string()
    }
}

#[test]
fn test_unversioned_tool_unaffected_by_current_version() {
    // Even after pinning a current version, an unversioned tool must
    // keep working. This is the "no gating" path.
    set_current_version("99.0.0");
    let tools_inst = UnversionedTools::default();
    let result =
        <UnversionedTools as ToolCaller>::call_tool(&tools_inst, "ping", &json!({})).unwrap();
    assert_eq!(result, json!("pong"));
}
