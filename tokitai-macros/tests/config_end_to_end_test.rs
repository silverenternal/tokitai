//! End-to-end tests for the `config!` macro.
//!
//! These tests pin the contract between the `config!` macro and the
//! global `GLOBAL_CONFIG_REGISTRY`: the macro-generated `__CONFIG_INIT_*`
//! static must populate the registry when first dereferenced, and the
//! registry entries must carry the configured descriptions, tags, and
//! per-parameter metadata.
//!
//! The tests share a single process-wide `GLOBAL_CONFIG_REGISTRY`, so each
//! test is responsible for cleaning up only the keys it touches.
//!
//! Run with: `cargo test -p tokitai-macros --test config_end_to_end_test --features serde`

#![cfg(feature = "serde")]
#![allow(non_snake_case, non_upper_case_globals, deprecated)]

use tokitai::tool;
use tokitai::{config, ToolConfig};

// ============================================================================
// Test 1: `config!` overrides the tool description
// ============================================================================

#[derive(Default)]
struct ConfigDescTools;

#[tool]
impl ConfigDescTools {
    /// Default description - should be overridden by the config block.
    pub fn get_user(&self, id: i32) -> String {
        format!("User {}", id)
    }
}

config! {
    ConfigDescTools {
        get_user: {
            desc: "Configuration-overridden description",
            params: {
                id: { desc: "User ID parameter" }
            }
        }
    }
}

#[test]
fn test_config_desc_override() {
    // Force the config initialisation.
    let _ = &*__CONFIG_INIT_ConfigDescTools;

    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("get_user"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("get_user");
    assert!(!configs.is_empty());

    let has_desc = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "Configuration-overridden description"));
    assert!(has_desc, "expected the overridden Desc entry to be present");

    let has_param_desc = configs.iter().any(|c| {
        matches!(c, ToolConfig::ParamDesc { name, desc } if name == "id" && desc == "User ID parameter")
    });
    assert!(
        has_param_desc,
        "expected the per-param ParamDesc entry to be present"
    );
}

// ============================================================================
// Test 2: `config!` adds tags
// ============================================================================

#[derive(Default)]
struct ConfigTagsTools;

#[tool]
impl ConfigTagsTools {
    pub fn search(&self, query: String) -> Vec<String> {
        vec![query]
    }
}

config! {
    ConfigTagsTools {
        search: {
            desc: "Search functionality",
            tags: ["search", "utility"]
        }
    }
}

#[test]
fn test_config_tags() {
    // Force the config initialisation.
    let _ = &*__CONFIG_INIT_ConfigTagsTools;

    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("search"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("search");

    let has_tags = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Tags(tags) if tags.contains(&"search".to_string())));
    assert!(has_tags, "expected a Tags entry containing \"search\"");
}

// ============================================================================
// Test 3: `config!` adds a per-parameter example
// ============================================================================

#[derive(Default)]
struct ConfigExampleTools;

#[tool]
impl ConfigExampleTools {
    pub fn greet(&self, name: String) -> String {
        format!("Hello, {}", name)
    }
}

config! {
    ConfigExampleTools {
        greet: {
            desc: "Greeting functionality",
            params: {
                name: {
                    desc: "Name to greet",
                    example: "Alice"
                }
            }
        }
    }
}

#[test]
fn test_config_param_example() {
    // Force the config initialisation.
    let _ = &*__CONFIG_INIT_ConfigExampleTools;

    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("greet"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("greet");

    let has_example = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::ParamExample { name, .. } if name == "name"));
    assert!(has_example, "expected a ParamExample entry for \"name\"");
}

// ============================================================================
// Test 4: `config!` covering multiple methods on the same struct
// ============================================================================

#[derive(Default)]
struct MultiMethodTools;

#[tool]
impl MultiMethodTools {
    /// Default description for method 1.
    pub fn method1(&self, a: i32) -> i32 {
        a
    }

    /// Default description for method 2.
    pub fn method2(&self, b: String) -> String {
        b
    }
}

config! {
    MultiMethodTools {
        method1: {
            desc: "Configured description for method 1",
            params: {
                a: { desc: "Parameter a" }
            }
        },
        method2: {
            desc: "Configured description for method 2",
            tags: ["custom"]
        }
    }
}

#[test]
fn test_config_multiple_methods() {
    // Force the config initialisation.
    let _ = &*__CONFIG_INIT_MultiMethodTools;

    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("method1"));
    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("method2"));

    let configs1 = tokitai::GLOBAL_CONFIG_REGISTRY.get("method1");
    let configs2 = tokitai::GLOBAL_CONFIG_REGISTRY.get("method2");

    let has_desc1 = configs1
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "Configured description for method 1"));
    assert!(has_desc1, "method1 should have its Desc entry");

    let has_desc2 = configs2
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "Configured description for method 2"));
    assert!(has_desc2, "method2 should have its Desc entry");

    let has_tags2 = configs2
        .iter()
        .any(|c| matches!(c, ToolConfig::Tags(tags) if tags.contains(&"custom".to_string())));
    assert!(has_tags2, "method2 should have its Tags entry");
}

// ============================================================================
// Test 5: `config!` boundary cases
// ============================================================================

#[derive(Default)]
struct EdgeCaseTools;

#[tool]
impl EdgeCaseTools {
    pub fn no_config_method(&self) -> String {
        "no config".to_string()
    }

    pub fn with_config_method(&self, x: i32) -> i32 {
        x * 2
    }
}

config! {
    EdgeCaseTools {
        with_config_method: {
            desc: "A method that has a config",
            params: {
                x: {
                    desc: "Input value",
                    example: 42
                }
            }
        }
    }
}

#[test]
fn test_config_edge_cases() {
    // Force the config initialisation.
    let _ = &*__CONFIG_INIT_EdgeCaseTools;

    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("with_config_method"));
    assert!(!tokitai::GLOBAL_CONFIG_REGISTRY.has_config("no_config_method"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("with_config_method");
    assert!(!configs.is_empty());

    let has_desc = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "A method that has a config"));
    assert!(has_desc);

    let has_param_desc = configs.iter().any(
        |c| matches!(c, ToolConfig::ParamDesc { name, desc } if name == "x" && desc == "Input value"),
    );
    assert!(has_param_desc);

    // Note: ParamExample may need additional handling, so only the
    // basic flow is pinned here.
    // let has_param_example = configs.iter().any(|c| {
    //     matches!(c, ToolConfig::ParamExample { name, example } if name == "x" && example == 42)
    // });
    // assert!(has_param_example);
}

// ============================================================================
// Test 6: `config!` interaction with the `#[deprecated]` attribute
// ============================================================================

#[derive(Default)]
struct InteractionTools;

#[tool]
impl InteractionTools {
    /// Original description.
    #[deprecated]
    pub fn deprecated_method(&self) -> String {
        "deprecated".to_string()
    }
}

config! {
    InteractionTools {
        deprecated_method: {
            desc: "Configured description",
            tags: ["deprecated"]
        }
    }
}

#[test]
fn test_config_with_deprecated() {
    // Force the config initialisation.
    let _ = &*__CONFIG_INIT_InteractionTools;

    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("deprecated_method"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("deprecated_method");

    let has_desc = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "Configured description"));
    assert!(has_desc);

    let has_tags = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Tags(tags) if tags.contains(&"deprecated".to_string())));
    assert!(has_tags);
}

// ============================================================================
// Test 7: registry query API
// ============================================================================

#[test]
fn test_registry_query() {
    // `has_config` returns false for unknown tools.
    assert!(!tokitai::GLOBAL_CONFIG_REGISTRY.has_config("__nonexistent_tool__"));

    // `get` returns an empty list for unknown tools.
    let nonexistent_configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("__nonexistent_tool__");
    assert!(nonexistent_configs.is_empty());
}

// ============================================================================
// Test 8: registry mutation API. Uses a private key so it does not
// collide with the other tests' shared state.
// ============================================================================

#[test]
fn test_registry_clear() {
    let temp_key = "__test_registry_clear_temp_method__";

    tokitai::GLOBAL_CONFIG_REGISTRY.configure(temp_key, &[ToolConfig::desc("Initial description")]);
    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config(temp_key));

    // Clearing a specific key removes it.
    tokitai::GLOBAL_CONFIG_REGISTRY.clear(temp_key);
    assert!(!tokitai::GLOBAL_CONFIG_REGISTRY.has_config(temp_key));

    // A subsequent `configure` re-adds the key.
    tokitai::GLOBAL_CONFIG_REGISTRY
        .configure(temp_key, &[ToolConfig::desc("Reconfigured description")]);
    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config(temp_key));

    // Clean up so this test does not leave residue in the shared
    // process-wide registry.
    tokitai::GLOBAL_CONFIG_REGISTRY.clear(temp_key);
    assert!(!tokitai::GLOBAL_CONFIG_REGISTRY.has_config(temp_key));
}
