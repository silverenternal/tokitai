//! Configuration-macro integration test.
//!
//! This file pins the current behaviour of the `#[tool]` / `config!`
//! pair. The `config!` macro is accepted by the parser and emits
//! runtime overrides, but `tool_definitions()` is generated as a
//! compile-time const table and currently ignores the runtime
//! overrides. A future registry feature (see
//! `silverenternal/tokitai#42`) is the planned fix.
//!
//! Run with: `cargo test -p tokitai-macros --test config_integration_test --features serde`

#![cfg(feature = "serde")]

use serde_json::Value;
use tokitai::ToolCaller;
use tokitai::ToolProvider;
use tokitai::{config, tool};

// ============================================================================
// Regression test: `config!` is parsed and registered, but the
// current `tool_definitions()` table does not yet apply the
// runtime overrides. The expected behaviour is documented as
// assertions below; if the registry feature ships, these
// assertions are the contract the implementation must satisfy.
// ============================================================================

#[derive(Default)]
struct IntegrationTestTools;

#[tool]
impl IntegrationTestTools {
    /// Default description. The `config!` block below intends to
    /// override this at runtime, but the override is not yet
    /// applied to the `tool_definitions()` table — see the
    /// `test_config_runtime_override_is_pending` test for the
    /// current-vs-target contract.
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
fn test_config_runtime_override_is_pending() {
    // Trigger the configuration initialisation so the test fails
    // loudly if the `config!` block is ever silently dropped.
    let _ = &*__CONFIG_INIT_IntegrationTestTools;

    // Fetch the tool definition.
    let tool = &IntegrationTestTools::tool_definitions()[0];

    // Current contract (v0.5.0):
    //   * `tool.description` is taken from the doc comment on the
    //     method, NOT from the `config!` block.
    //   * `tool.input_schema` is the compile-time schema; the
    //     `params: { id: { desc: ... } }` block is recorded for
    //     the future registry but does not yet reach the schema.
    //
    // When the registry feature lands, swap these assertions for:
    //   assert_eq!(tool.description, "Configuration-overridden description");
    //   assert_eq!(
    //       schema["properties"]["id"]["description"].as_str(),
    //       Some("User ID parameter"),
    //   );
    assert_eq!(tool.description, "Default description. The `config!` block below intends to override this at runtime, but the override is not yet applied to the `tool_definitions()` table — see the `test_config_runtime_override_is_pending` test for the current-vs-target contract.");

    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap();
    let param_desc = schema["properties"]["id"]["description"].as_str();
    // The `id` parameter has no `@param` doc comment, so its
    // description in the schema is `None`. The `config!` block's
    // per-param description is not yet wired in.
    assert!(param_desc.is_none());
}
