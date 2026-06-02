//! Configuration-macro integration test.
//!
//! As of 0.5.1 the `config!` macro fully applies its overrides to the
//! `tool_definitions()` table: the `desc:` override wins over the
//! method's doc comment, and per-param `desc:` overrides reach the
//! JSON Schema's `properties.<name>.description` field. This test
//! pins that contract.
//!
//! Run with: `cargo test -p tokitai-macros --test config_integration_test --features serde`

#![cfg(feature = "serde")]
#![allow(non_snake_case, non_upper_case_globals)]

use serde_json::Value;
use tokitai::ToolProvider;
use tokitai::{config, tool};

// ============================================================================
// The `config!` block below overrides both the method description
// and the `id` parameter's description on the schema.
// ============================================================================

#[derive(Default)]
struct IntegrationTestTools;

#[tool]
impl IntegrationTestTools {
    /// Method doc comment kept as a fallback. When a `config!` block
    /// supplies a `desc:`, the registry version wins.
    pub fn get_user(&self, id: i32) -> String {
        format!("User {}", id)
    }
}

config! {
    IntegrationTestTools {
        get_user: {
            desc: "Configuration-overridden description",
            params: {
                id: { desc: "User ID parameter" }
            }
        }
    }
}

#[test]
fn test_config_runtime_override_is_applied() {
    // Trigger the configuration initialisation so the test fails
    // loudly if the `config!` block is ever silently dropped.
    let _ = &*__CONFIG_INIT_IntegrationTestTools;

    // Fetch the tool definition.
    let tool = &IntegrationTestTools::tool_definitions()[0];

    // 1. The `config!`-supplied `desc:` wins over the method doc comment.
    assert_eq!(tool.description, "Configuration-overridden description");

    // 2. The per-param `desc:` is wired into the JSON Schema.
    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap();
    assert_eq!(
        schema["properties"]["id"]["description"].as_str(),
        Some("User ID parameter"),
    );

    // 3. The per-param `desc:` is also still recorded in the global
    //    registry (so downstream code can read it without parsing the
    //    schema back out).
    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("get_user");
    let has_param_desc = configs.iter().any(|c| {
        matches!(c, tokitai::ToolConfig::ParamDesc { name, desc } if name == "id" && desc == "User ID parameter")
    });
    assert!(
        has_param_desc,
        "the per-param desc should be in the registry"
    );
}
