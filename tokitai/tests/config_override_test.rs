//! T-002: integration test that pins the configuration priority table.
//!
//! The acceptance criterion for T-002 is that `tokitai! { desc: "..." }`
//! must NOT override a description supplied via `#[tool(desc = "...")]`
//! at compile time. Lower-priority sources (doc comments, the synthesized
//! default) stay open to runtime override.
//!
//! These tests use the `tokitai` user-facing crate so the macro side and
//! the runtime side are both exercised end-to-end.

#![cfg(feature = "serde")]
#![allow(non_snake_case, non_upper_case_globals)]

use tokitai::{config, tool, ToolProvider};

// ============================================================================
// Case 1: `#[tool(desc = "...")]` is the compile-time winner; the runtime
// `tokitai!` config block must NOT override it.
// ============================================================================

#[derive(Default)]
struct ExplicitDescTools;

#[tool]
impl ExplicitDescTools {
    #[tool(
        desc = "Returns a static greeting String. The description is supplied at compile time and is locked from runtime override."
    )]
    pub fn greet(&self) -> String {
        "hi".to_string()
    }
}

// Intentional runtime override attempt — should be ignored because the
// description was supplied explicitly at compile time.
config! {
    ExplicitDescTools {
        greet: { desc: "Runtime attempt to override attribute" }
    }
}

#[test]
fn test_tool_attr_desc_is_not_overridden_by_tokitai_config() {
    // Force the registry to materialize.
    let _ = &*__CONFIG_INIT_ExplicitDescTools;

    let tool = ExplicitDescTools::tool_definitions()
        .iter()
        .find(|t| t.name == "greet")
        .expect("greet tool missing");
    assert_eq!(tool.description, "Returns a static greeting String. The description is supplied at compile time and is locked from runtime override.");
    assert!(
        tool.description_explicit,
        "explicitly-attributed descriptions must be flagged so the runtime respects the priority table",
    );
}

// ============================================================================
// Case 2: a doc-comment description IS overridable by the runtime
// `tokitai!` config. This keeps the existing `#[tool]` ergonomics where
// doc comments are the "default for compile-time" path.
// ============================================================================

#[derive(Default)]
struct DocCommentTools;

#[tool]
impl DocCommentTools {
    /// Doc-comment description (no `#[tool(desc = "...")]`)
    pub fn ping(&self) -> String {
        "pong".to_string()
    }
}

config! {
    DocCommentTools {
        ping: { desc: "Runtime override of doc comment" }
    }
}

#[test]
fn test_doc_comment_description_is_overridable() {
    let _ = &*__CONFIG_INIT_DocCommentTools;

    let tool = DocCommentTools::tool_definitions()
        .iter()
        .find(|t| t.name == "ping")
        .expect("ping tool missing");
    assert_eq!(tool.description, "Runtime override of doc comment");
    assert!(
        !tool.description_explicit,
        "doc-comment descriptions must NOT be flagged as explicit",
    );
}

// ============================================================================
// Case 3: when neither `#[tool(desc)]` nor a doc comment is present,
// the synthesized default description is open to runtime override.
// ============================================================================

#[derive(Default)]
struct DefaultDescTools;

#[tool]
impl DefaultDescTools {
    pub fn no_docs(&self) -> String {
        "result".to_string()
    }
}

config! {
    DefaultDescTools {
        no_docs: { desc: "Runtime-supplied default override" }
    }
}

#[test]
fn test_synthesized_default_description_is_overridable() {
    let _ = &*__CONFIG_INIT_DefaultDescTools;

    let tool = DefaultDescTools::tool_definitions()
        .iter()
        .find(|t| t.name == "no_docs")
        .expect("no_docs tool missing");
    assert_eq!(tool.description, "Runtime-supplied default override");
    assert!(!tool.description_explicit);
}

// ============================================================================
// Case 4: per-parameter overrides remain runtime-only (a `param_desc`
// from `tokitai!` is allowed regardless of whether `#[tool(desc)]` was
// used; it is NOT a tool-level description).
// ============================================================================

#[derive(Default)]
struct PerParamOverrideTools;

#[tool]
impl PerParamOverrideTools {
    #[tool(
        desc = "Returns the supplied id parameter as a String. Description is locked from runtime config override."
    )]
    pub fn action(&self, id: String) -> String {
        id
    }
}

config! {
    PerParamOverrideTools {
        action: {
            params: {
                id: { desc: "runtime param desc" }
            }
        }
    }
}

#[test]
fn test_per_param_desc_runtime_override_still_works() {
    let _ = &*__CONFIG_INIT_PerParamOverrideTools;

    let tool = PerParamOverrideTools::tool_definitions()
        .iter()
        .find(|t| t.name == "action")
        .expect("action tool missing");

    // Tool-level description is still locked.
    assert_eq!(tool.description, "Returns the supplied id parameter as a String. Description is locked from runtime config override.");

    // Per-parameter description IS overridable at runtime, even when the
    // tool-level description is locked.
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    assert_eq!(
        schema["properties"]["id"]["description"].as_str(),
        Some("runtime param desc"),
    );
}
