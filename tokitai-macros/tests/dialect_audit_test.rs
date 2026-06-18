//! T-012: Compile-time dialect correctness audit.
//!
//! Every LLM tool-calling provider has slightly different
//! rules for what a valid JSON Schema looks like. This file
//! asserts that the macro:
//!
//! 1. Accepts the `#[tool(dialect = "...")]` attribute and
//!    picks the right rule set (positive cases).
//! 2. Refuses to emit a schema that the chosen dialect
//!    cannot accept, with a diagnostic anchored at the
//!    user-written token (negative cases).
//!
//! The runtime `audit` function is unit-tested in
//! `tokitai-macros/src/tool/schema/dialect.rs`; this file
//! covers the macro integration path (the
//! `compile_error!` plumbing).

use tokitai::tool;
use tokitai::ToolProvider;

// ---------------------------------------------------------------------------
// Positive cases
// ---------------------------------------------------------------------------

/// Positive case 1: default dialect (`mcp`) accepts a
/// plain object. No audit should fire and the macro should
/// produce a working `tool_definitions()`.
#[derive(Default)]
pub struct PositiveMcp;

#[tool]
impl PositiveMcp {
    /// Echo a string back.
    pub fn echo(&self, message: String) -> String {
        message
    }
}

#[test]
fn positive_default_mcp_dialect_compiles_and_runs() {
    let p = PositiveMcp;
    let tools = <PositiveMcp as ToolProvider>::tool_definitions();
    assert_eq!(tools.len(), 1);
    let result = p
        .call_tool("echo", &serde_json::json!({"message": "hi"}))
        .unwrap();
    assert_eq!(result, "hi");
}

/// Positive case 2: `dialect = "openai-strict"` on a
/// well-formed schema compiles and emits the expected tool
/// definitions.
#[derive(Default)]
pub struct PositiveOpenAi;

#[tool(dialect = "openai-strict")]
impl PositiveOpenAi {
    /// Add two integers.
    pub fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }
}

#[test]
fn positive_openai_strict_dialect_compiles() {
    let tools = <PositiveOpenAi as ToolProvider>::tool_definitions();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "add");
}

/// Positive case 3: `dialect = "anthropic"` on a
/// well-formed schema compiles. Anthropic's stricter rule
/// (`additionalProperties: false` required) is satisfied by
/// the macro's default object shape — the codegen path
/// always emits `additionalProperties: false` on root
/// objects, so the audit does not fire.
#[derive(Default)]
pub struct PositiveAnthropic;

#[tool(dialect = "anthropic")]
impl PositiveAnthropic {
    /// Lookup a user by id.
    pub fn get_user(&self, id: String) -> String {
        format!("user-{}", id)
    }
}

#[test]
fn positive_anthropic_dialect_compiles() {
    let tools = <PositiveAnthropic as ToolProvider>::tool_definitions();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "get_user");
}

// ---------------------------------------------------------------------------
// Negative cases — we cannot compile these in the same
// crate because the macro will reject them at expansion
// time. Instead, the matching `tests/ui/*.rs` fixtures
// exercise the negative path through trybuild, and we
// verify the audit logic itself via the unit tests in
// `tokitai-macros/src/tool/schema/dialect.rs`.
//
// The negative fixtures are:
//   * tests/ui/15_unknown_dialect.rs       — unknown dialect name (E0030)
//   * tests/ui/16_openai_tuple_param.rs    — tuple param fails OA-3 (E0030)
//   * tests/ui/17_openai_any_param.rs      — `Option<Value>` fails OA-2 (E0030)
//   * tests/ui/18_anthropic_extra_props.rs — explicit additional_properties=true fails AN-1 (E0030)
//   * tests/ui/19_mcp_missing_type.rs      — raw `serde_json::Value` param fails MCP-1 (E0030)
//
// This test file only embeds the positive cases. The
// negative path is exercised by trybuild in `ui_tests.rs`.
// ---------------------------------------------------------------------------

#[test]
fn dialect_name_aliases_are_accepted() {
    // T-012 acceptance: `openai` and `claude` are documented
    // aliases for `openai-strict` and `anthropic`
    // respectively. They must compile to the same tool
    // definitions, so we just verify the macro accepts each
    // alias name without producing an `E0030`.

    #[derive(Default)]
    pub struct AliasedOpenai;

    #[tool(dialect = "openai")]
    impl AliasedOpenai {
        /// Doc
        pub fn f(&self) -> i32 {
            1
        }
    }

    #[derive(Default)]
    pub struct AliasedClaude;

    #[tool(dialect = "claude")]
    impl AliasedClaude {
        /// Doc
        pub fn g(&self) -> i32 {
            2
        }
    }

    // Reference each so the unused-struct lint does not strip
    // the macros during test build.
    let _ = AliasedOpenai;
    let _ = AliasedClaude;
}
