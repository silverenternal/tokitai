//! Tool configuration system for runtime tool definition customization.
//!
//! This module provides the configuration types and registry for customizing
//! tool definitions at runtime through the `tokitai!` configuration macro.
//!
//! ## Usage
//!
//! ```rust
//! use tokitai_core::{ToolConfig, ToolConfigRegistry};
//!
//! // Define tool configurations
//! let configs = vec![
//!     ToolConfig::Desc("Custom tool description".to_string()),
//!     ToolConfig::Tags(vec!["useful".to_string(), "ai".to_string()]),
//!     ToolConfig::ParamDesc {
//!         name: "user_id".to_string(),
//!         desc: "The unique identifier for the user".to_string(),
//!     },
//! ];
//!
//! // Register configurations for a tool
//! let registry = ToolConfigRegistry::default();
//! registry.configure("get_user", &configs);
//! ```

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::{Arc, RwLock};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Configuration item for tool customization.
///
/// This enum represents different types of configurations that can be applied
/// to tools and their parameters.
///
/// # Example
///
/// ```rust
/// use tokitai_core::ToolConfig;
///
/// // Override the description of a tool:
/// let c1 = ToolConfig::Desc("Look up a user by id".to_string());
///
/// // Add per-parameter metadata to the generated JSON Schema:
/// let c2 = ToolConfig::ParamMin {
///     name: "age".to_string(),
///     min: 0.0,
/// };
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ToolConfig {
    /// Tool description override
    Desc(String),
    /// Tool tags for categorization
    Tags(Vec<String>),
    /// Parameter description override
    ParamDesc {
        /// Name of the parameter whose description is being overridden.
        name: String,
        /// New description for the parameter.
        desc: String,
    },
    /// Parameter example value
    ParamExample {
        /// Name of the parameter this example applies to.
        name: String,
        /// Example value to surface in the generated JSON Schema.
        example: serde_json::Value,
    },
    /// Parameter default value
    ParamDefault {
        /// Name of the parameter receiving the default value.
        name: String,
        /// Default value to surface in the generated JSON Schema.
        default: serde_json::Value,
    },
    /// Parameter required flag
    ParamRequired {
        /// Name of the parameter whose `required` flag is being toggled.
        name: String,
        /// New `required` value (`true` enforces the parameter).
        required: bool,
    },
    /// Parameter minimum value (for numbers)
    ParamMin {
        /// Name of the parameter receiving the minimum constraint.
        name: String,
        /// Lower bound for the parameter (inclusive).
        min: f64,
    },
    /// Parameter maximum value (for numbers)
    ParamMax {
        /// Name of the parameter receiving the maximum constraint.
        name: String,
        /// Upper bound for the parameter (inclusive).
        max: f64,
    },
    /// Parameter minimum length (for strings)
    ParamMinLength {
        /// Name of the parameter receiving the minimum-length constraint.
        name: String,
        /// Minimum string length (inclusive).
        min_length: u64,
    },
    /// Parameter maximum length (for strings)
    ParamMaxLength {
        /// Name of the parameter receiving the maximum-length constraint.
        name: String,
        /// Maximum string length (inclusive).
        max_length: u64,
    },
    /// Parameter regex pattern (for strings)
    ParamPattern {
        /// Name of the parameter receiving the pattern constraint.
        name: String,
        /// Regular expression the value must match.
        pattern: String,
    },
    /// Parameter minimum items (for arrays)
    ParamMinItems {
        /// Name of the parameter receiving the minimum-items constraint.
        name: String,
        /// Minimum number of array elements.
        min_items: u64,
    },
    /// Parameter maximum items (for arrays)
    ParamMaxItems {
        /// Name of the parameter receiving the maximum-items constraint.
        name: String,
        /// Maximum number of array elements.
        max_items: u64,
    },
    /// Parameter multiple of (for numbers)
    ParamMultipleOf {
        /// Name of the parameter receiving the multiple-of constraint.
        name: String,
        /// Value the parameter must be a multiple of.
        multiple_of: f64,
    },
}

impl ToolConfig {
    /// Create a description configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::desc("Look up a user by id");
    /// assert!(matches!(c, ToolConfig::Desc(_)));
    /// ```
    pub fn desc<S: Into<String>>(desc: S) -> Self {
        ToolConfig::Desc(desc.into())
    }

    /// Create a tags configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::tags(vec!["read".to_string(), "user".to_string()]);
    /// assert!(matches!(c, ToolConfig::Tags(_)));
    /// ```
    pub fn tags(tags: Vec<String>) -> Self {
        ToolConfig::Tags(tags)
    }

    /// Create a parameter description configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_desc("id", "User identifier");
    /// assert!(matches!(c, ToolConfig::ParamDesc { .. }));
    /// ```
    pub fn param_desc<N: Into<String>, D: Into<String>>(name: N, desc: D) -> Self {
        ToolConfig::ParamDesc {
            name: name.into(),
            desc: desc.into(),
        }
    }

    /// Create a parameter example configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    /// use serde_json::json;
    ///
    /// let c = ToolConfig::param_example("id", json!("user-42"));
    /// assert!(matches!(c, ToolConfig::ParamExample { .. }));
    /// ```
    pub fn param_example<N: Into<String>>(name: N, example: serde_json::Value) -> Self {
        ToolConfig::ParamExample {
            name: name.into(),
            example,
        }
    }

    /// Create a parameter default configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    /// use serde_json::json;
    ///
    /// let c = ToolConfig::param_default("limit", json!(10));
    /// assert!(matches!(c, ToolConfig::ParamDefault { .. }));
    /// ```
    pub fn param_default<N: Into<String>>(name: N, default: serde_json::Value) -> Self {
        ToolConfig::ParamDefault {
            name: name.into(),
            default,
        }
    }

    /// Create a parameter required configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_required("id", true);
    /// assert!(matches!(c, ToolConfig::ParamRequired { required: true, .. }));
    /// ```
    pub fn param_required<N: Into<String>>(name: N, required: bool) -> Self {
        ToolConfig::ParamRequired {
            name: name.into(),
            required,
        }
    }

    /// Create a parameter minimum configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_min("age", 0.0);
    /// assert!(matches!(c, ToolConfig::ParamMin { min, .. } if min == 0.0));
    /// ```
    pub fn param_min<N: Into<String>>(name: N, min: f64) -> Self {
        ToolConfig::ParamMin {
            name: name.into(),
            min,
        }
    }

    /// Create a parameter maximum configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_max("age", 120.0);
    /// assert!(matches!(c, ToolConfig::ParamMax { max, .. } if max == 120.0));
    /// ```
    pub fn param_max<N: Into<String>>(name: N, max: f64) -> Self {
        ToolConfig::ParamMax {
            name: name.into(),
            max,
        }
    }

    /// Create a parameter minimum length configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_min_length("username", 3);
    /// assert!(matches!(c, ToolConfig::ParamMinLength { min_length: 3, .. }));
    /// ```
    pub fn param_min_length<N: Into<String>>(name: N, min_length: u64) -> Self {
        ToolConfig::ParamMinLength {
            name: name.into(),
            min_length,
        }
    }

    /// Create a parameter maximum length configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_max_length("username", 32);
    /// assert!(matches!(c, ToolConfig::ParamMaxLength { max_length: 32, .. }));
    /// ```
    pub fn param_max_length<N: Into<String>>(name: N, max_length: u64) -> Self {
        ToolConfig::ParamMaxLength {
            name: name.into(),
            max_length,
        }
    }

    /// Create a parameter pattern configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_pattern("email", r"^[^@]+@[^@]+$");
    /// assert!(matches!(c, ToolConfig::ParamPattern { .. }));
    /// ```
    pub fn param_pattern<N: Into<String>, P: Into<String>>(name: N, pattern: P) -> Self {
        ToolConfig::ParamPattern {
            name: name.into(),
            pattern: pattern.into(),
        }
    }

    /// Create a parameter minimum items configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_min_items("tags", 1);
    /// assert!(matches!(c, ToolConfig::ParamMinItems { min_items: 1, .. }));
    /// ```
    pub fn param_min_items<N: Into<String>>(name: N, min_items: u64) -> Self {
        ToolConfig::ParamMinItems {
            name: name.into(),
            min_items,
        }
    }

    /// Create a parameter maximum items configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_max_items("tags", 10);
    /// assert!(matches!(c, ToolConfig::ParamMaxItems { max_items: 10, .. }));
    /// ```
    pub fn param_max_items<N: Into<String>>(name: N, max_items: u64) -> Self {
        ToolConfig::ParamMaxItems {
            name: name.into(),
            max_items,
        }
    }

    /// Create a parameter multiple of configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfig;
    ///
    /// let c = ToolConfig::param_multiple_of("step", 0.5);
    /// assert!(matches!(c, ToolConfig::ParamMultipleOf { multiple_of, .. } if multiple_of == 0.5));
    /// ```
    pub fn param_multiple_of<N: Into<String>>(name: N, multiple_of: f64) -> Self {
        ToolConfig::ParamMultipleOf {
            name: name.into(),
            multiple_of,
        }
    }
}

/// Named source layers that can supply a tool's description.
///
/// The order is the **priority order** for description resolution: index
/// 0 wins, and the `Default` synthesized description is the fallback.
///
/// This enum is the single source of truth for the priority table;
/// [`CONFIG_PRIORITY_ORDER`] renders it as a fixed-size array so the
/// `#[tool]` macro and the user-facing docs can both reference the same
/// `const`.
///
/// # Example
///
/// ```rust
/// use tokitai_core::config::ConfigLayer;
///
/// // `#[tool(desc = "...")]` wins; the `tokitai!` config is one notch below.
/// assert!(ConfigLayer::ToolAttrDesc.priority() < ConfigLayer::TokitaiConfig.priority());
/// assert!(ConfigLayer::DocComment.priority() < ConfigLayer::TokitaiConfig.priority());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigLayer {
    /// `#[tool(desc = "...")]` on the method itself (compile-time, attribute).
    ToolAttrDesc = 0,
    /// `///` doc comment lines above the method (compile-time, doc).
    DocComment = 1,
    /// `tokitai! { ... desc: "..." }` runtime override.
    TokitaiConfig = 2,
    /// The synthesized default (`"调用 <method> 方法"` / fallback string).
    Default = 3,
}

impl ConfigLayer {
    /// Lower numbers win. Returns 0 for the highest priority, 3 for the
    /// fallback.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::config::ConfigLayer;
    ///
    /// assert_eq!(ConfigLayer::ToolAttrDesc.priority(), 0);
    /// assert_eq!(ConfigLayer::DocComment.priority(), 1);
    /// assert_eq!(ConfigLayer::TokitaiConfig.priority(), 2);
    /// assert_eq!(ConfigLayer::Default.priority(), 3);
    /// ```
    pub const fn priority(self) -> u8 {
        self as u8
    }

    /// Stable string label suitable for diagnostics and docs.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::config::ConfigLayer;
    ///
    /// assert_eq!(ConfigLayer::ToolAttrDesc.label(), "#[tool(desc = \"...\")]");
    /// assert_eq!(ConfigLayer::DocComment.label(), "doc comment");
    /// assert_eq!(ConfigLayer::TokitaiConfig.label(), "tokitai! config");
    /// assert_eq!(ConfigLayer::Default.label(), "synthesized default");
    /// ```
    pub const fn label(self) -> &'static str {
        match self {
            ConfigLayer::ToolAttrDesc => "#[tool(desc = \"...\")]",
            ConfigLayer::DocComment => "doc comment",
            ConfigLayer::TokitaiConfig => "tokitai! config",
            ConfigLayer::Default => "synthesized default",
        }
    }
}

/// Frozen priority table for tool-description resolution.
///
/// Index 0 is the highest-priority source (wins on conflict); the last
/// index is the lowest (used only as a fallback). This table is the
/// `const fn` counterpart of the public doc table in
/// [`crate::CONFIG_PRIORITY_DOC`].
///
/// # Why a `const fn`?
///
/// `#[tool]` and the docs used to live in two places — a doc comment in
/// `tokitai-macros/src/lib.rs` and the per-doc-file prose in
/// `docs/USAGE.md` / `docs/ADVANCED_USAGE.md` — which drifted apart.
/// Tying the macro and the docs to this single array means there is
/// exactly one source of truth, and a unit test can compare the
/// rendered array against the docs verbatim.
///
/// # Example
///
/// ```rust
/// use tokitai_core::config::{CONFIG_PRIORITY_ORDER, ConfigLayer};
///
/// // Highest-priority layer first.
/// assert_eq!(CONFIG_PRIORITY_ORDER[0], ConfigLayer::ToolAttrDesc);
/// assert_eq!(CONFIG_PRIORITY_ORDER[1], ConfigLayer::DocComment);
/// assert_eq!(CONFIG_PRIORITY_ORDER[2], ConfigLayer::TokitaiConfig);
/// assert_eq!(CONFIG_PRIORITY_ORDER[3], ConfigLayer::Default);
/// ```
pub const CONFIG_PRIORITY_ORDER: [ConfigLayer; 4] = [
    ConfigLayer::ToolAttrDesc,
    ConfigLayer::DocComment,
    ConfigLayer::TokitaiConfig,
    ConfigLayer::Default,
];

/// Render the priority table as a markdown-flavoured `&'static [&'static str]`.
///
/// The output is suitable for inlining into `docs/USAGE.md` and
/// `docs/ADVANCED_USAGE.md` via a build script or by hand. Each row
/// reads `1. <label>`, matching the user-facing docs.
///
/// # Example
///
/// ```rust
/// use tokitai_core::config::config_priority_table_md;
///
/// let table = config_priority_table_md();
/// assert!(table[0].contains("#[tool(desc"));
/// assert!(table[3].contains("synthesized default"));
/// ```
pub const fn config_priority_table_md() -> [&'static str; 4] {
    [
        "1. `#[tool(desc = \"...\")]` (compile-time, attribute-supplied) — **wins** on conflict",
        "2. doc comment (`///` lines above the method) — used if no `#[tool(desc)]` is present",
        "3. tokitai! config block (`GLOBAL_CONFIG_REGISTRY`) — does **not** override an explicit `#[tool(desc)]`",
        "4. synthesized default (e.g. `\"调用 <method> 方法\"`) — last-resort fallback",
    ]
}

/// Decide whether a runtime `tokitai!` config layer is allowed to override
/// a description that was supplied by a higher-priority layer.
///
/// `compile_time_winner` is the priority number of the layer that
/// supplied the description baked into the `ToolDefinition` at compile
/// time; `runtime_layer` is the layer that's trying to override it.
/// Returns `true` iff the override is permitted.
///
/// This is the single function that encodes the priority rules; both
/// the `#[tool]` macro and runtime callers route through it.
///
/// # Example
///
/// ```rust
/// use tokitai_core::config::{can_override, ConfigLayer};
///
/// // Compile-time `#[tool(desc)]` (priority 0) is never overridable.
/// assert!(!can_override(ConfigLayer::ToolAttrDesc.priority(), ConfigLayer::TokitaiConfig));
/// // Doc comments (priority 1) outrank the runtime layer (priority 2).
/// assert!(!can_override(ConfigLayer::DocComment.priority(), ConfigLayer::TokitaiConfig));
/// // The default fallback (priority 3) IS overridable.
/// assert!(can_override(ConfigLayer::Default.priority(), ConfigLayer::TokitaiConfig));
/// ```
pub const fn can_override(compile_time_winner: u8, runtime_layer: ConfigLayer) -> bool {
    // The runtime layer wins only when it has strictly higher priority
    // (lower number) than whatever the compile-time layer settled on.
    runtime_layer.priority() < compile_time_winner
}

/// Runtime registry for tool configurations.
///
/// This registry stores configurations applied via the `tokitai!` macro
/// and provides methods to query and apply them to tool definitions.
///
/// ## Thread Safety
///
/// `ToolConfigRegistry` is thread-safe and can be shared across threads.
/// It uses `RwLock` for efficient concurrent read access.
///
/// ## Example
///
/// ```rust
/// use tokitai_core::{ToolConfig, ToolConfigRegistry};
///
/// let registry = ToolConfigRegistry::default();
///
/// // Configure a tool
/// registry.configure("get_user", &[
///     ToolConfig::Desc("Get user information".to_string()),
///     ToolConfig::param_desc("id", "User ID"),
/// ]);
///
/// // Query configurations
/// let configs = registry.get("get_user");
/// assert!(!configs.is_empty());
/// ```
#[derive(Debug, Default, Clone)]
pub struct ToolConfigRegistry {
    configs: Arc<RwLock<HashMap<String, Vec<ToolConfig>>>>,
}

impl ToolConfigRegistry {
    /// Create a new empty registry.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::ToolConfigRegistry;
    ///
    /// let registry = ToolConfigRegistry::new();
    /// assert!(!registry.has_config("any_tool"));
    /// ```
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register configurations for a tool.
    ///
    /// This method stores the configuration items for the specified tool name.
    /// Multiple calls for the same tool will append configurations.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - The name of the tool to configure
    /// * `configs` - Slice of configuration items to apply
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::{ToolConfig, ToolConfigRegistry};
    ///
    /// let registry = ToolConfigRegistry::default();
    /// registry.configure("get_user", &[
    ///     ToolConfig::Desc("Custom description".to_string()),
    /// ]);
    /// ```
    pub fn configure(&self, tool_name: &str, configs: &[ToolConfig]) {
        let mut map = self.configs.write().unwrap();
        let entry = map.entry(tool_name.to_string()).or_default();
        entry.extend_from_slice(configs);
    }

    /// Get all configurations for a tool.
    ///
    /// Returns a clone of the configuration vector for the specified tool.
    /// Returns an empty vector if no configurations are registered.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - The name of the tool to query
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::{ToolConfig, ToolConfigRegistry};
    ///
    /// let registry = ToolConfigRegistry::default();
    /// registry.configure("get_user", &[
    ///     ToolConfig::Desc("Description".to_string()),
    /// ]);
    ///
    /// let configs = registry.get("get_user");
    /// assert_eq!(configs.len(), 1);
    /// ```
    pub fn get(&self, tool_name: &str) -> Vec<ToolConfig> {
        let map = self.configs.read().unwrap();
        map.get(tool_name).cloned().unwrap_or_default()
    }

    /// Check if a tool has any configurations registered.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - The name of the tool to check
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::{ToolConfig, ToolConfigRegistry};
    ///
    /// let registry = ToolConfigRegistry::default();
    /// assert!(!registry.has_config("unknown_tool"));
    ///
    /// registry.configure("get_user", &[
    ///     ToolConfig::Desc("Description".to_string()),
    /// ]);
    /// assert!(registry.has_config("get_user"));
    /// ```
    pub fn has_config(&self, tool_name: &str) -> bool {
        let map = self.configs.read().unwrap();
        map.contains_key(tool_name)
    }

    /// Clear all configurations for a specific tool.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - The name of the tool to clear configurations for
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::{ToolConfig, ToolConfigRegistry};
    ///
    /// let registry = ToolConfigRegistry::default();
    /// registry.configure("get_user", &[
    ///     ToolConfig::Desc("Description".to_string()),
    /// ]);
    ///
    /// registry.clear("get_user");
    /// assert!(!registry.has_config("get_user"));
    /// ```
    pub fn clear(&self, tool_name: &str) {
        let mut map = self.configs.write().unwrap();
        map.remove(tool_name);
    }

    /// Clear all configurations for all tools.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::{ToolConfig, ToolConfigRegistry};
    ///
    /// let registry = ToolConfigRegistry::default();
    /// registry.configure("tool1", &[ToolConfig::Desc("Desc 1".to_string())]);
    /// registry.configure("tool2", &[ToolConfig::Desc("Desc 2".to_string())]);
    ///
    /// registry.clear_all();
    /// assert!(!registry.has_config("tool1"));
    /// assert!(!registry.has_config("tool2"));
    /// ```
    pub fn clear_all(&self) {
        let mut map = self.configs.write().unwrap();
        map.clear();
    }
}

/// Global registry instance for tool configurations.
///
/// This is the default registry used by the `tokitai!` configuration macro.
/// Tools can access this registry to apply configurations to their definitions.
///
/// # Initialization Order
///
/// To avoid deadlocks, the initialization order is:
/// 1. `GLOBAL_CONFIG_REGISTRY` is initialized first (accessed by tool definitions)
/// 2. Tool definitions are initialized second (may access config registry)
///
/// # Safety
/// Do NOT call `tool_definitions()` inside a `tokitai!` configuration macro,
/// as this will cause a deadlock. The compiler cannot detect this pattern.
///
/// # Example
///
/// ```rust
/// use tokitai_core::{ToolConfig, ToolConfigRegistry};
///
/// let registry = ToolConfigRegistry::default();
/// registry.configure("get_user", &[
///     ToolConfig::Desc("Description".to_string()),
/// ]);
///
/// let configs = registry.get("get_user");
/// assert_eq!(configs.len(), 1);
/// ```
pub static GLOBAL_CONFIG_REGISTRY: LazyLock<ToolConfigRegistry> =
    LazyLock::new(ToolConfigRegistry::new);

/// Macro for compile-time deadlock detection.
///
/// This macro provides a compile-time check to help detect potential deadlocks
/// when using the configuration system.
///
/// # Usage
///
/// ```rust
/// use tokitai_core::assert_no_config_deadlock;
///
/// // Place this at the beginning of your tool implementation
/// assert_no_config_deadlock!();
/// ```
#[macro_export]
macro_rules! assert_no_config_deadlock {
    () => {
        // Compile-time check placeholder
        // Currently serves as documentation for the initialization order requirement
        // Future versions may add actual compile-time assertions
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_config_desc() {
        let config = ToolConfig::desc("Test description");
        match config {
            ToolConfig::Desc(desc) => assert_eq!(desc, "Test description"),
            _ => panic!("Expected Desc variant"),
        }
    }

    #[test]
    fn test_tool_config_tags() {
        let config = ToolConfig::tags(vec!["tag1".to_string(), "tag2".to_string()]);
        match config {
            ToolConfig::Tags(tags) => {
                assert_eq!(tags.len(), 2);
                assert_eq!(tags[0], "tag1");
                assert_eq!(tags[1], "tag2");
            }
            _ => panic!("Expected Tags variant"),
        }
    }

    #[test]
    fn test_tool_config_param_desc() {
        let config = ToolConfig::param_desc("param1", "Parameter description");
        match config {
            ToolConfig::ParamDesc { name, desc } => {
                assert_eq!(name, "param1");
                assert_eq!(desc, "Parameter description");
            }
            _ => panic!("Expected ParamDesc variant"),
        }
    }

    #[test]
    fn test_tool_config_param_example() {
        let example = serde_json::json!("example_value");
        let config = ToolConfig::param_example("param1", example.clone());
        match config {
            ToolConfig::ParamExample { name, example: ex } => {
                assert_eq!(name, "param1");
                assert_eq!(ex, example);
            }
            _ => panic!("Expected ParamExample variant"),
        }
    }

    #[test]
    fn test_registry_configure_and_get() {
        let registry = ToolConfigRegistry::default();

        registry.configure(
            "test_tool",
            &[
                ToolConfig::desc("Test description"),
                ToolConfig::param_desc("id", "ID parameter"),
            ],
        );

        let configs = registry.get("test_tool");
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_registry_has_config() {
        let registry = ToolConfigRegistry::default();

        assert!(!registry.has_config("nonexistent"));

        registry.configure("test_tool", &[ToolConfig::desc("Test")]);
        assert!(registry.has_config("test_tool"));
    }

    #[test]
    fn test_registry_clear() {
        let registry = ToolConfigRegistry::default();

        registry.configure("test_tool", &[ToolConfig::desc("Test")]);
        assert!(registry.has_config("test_tool"));

        registry.clear("test_tool");
        assert!(!registry.has_config("test_tool"));
    }

    #[test]
    fn test_registry_clear_all() {
        let registry = ToolConfigRegistry::default();

        registry.configure("tool1", &[ToolConfig::desc("Test 1")]);
        registry.configure("tool2", &[ToolConfig::desc("Test 2")]);

        registry.clear_all();

        assert!(!registry.has_config("tool1"));
        assert!(!registry.has_config("tool2"));
    }

    #[test]
    fn test_registry_multiple_configure() {
        let registry = ToolConfigRegistry::default();

        registry.configure("test_tool", &[ToolConfig::desc("First")]);
        registry.configure("test_tool", &[ToolConfig::param_desc("id", "ID")]);

        let configs = registry.get("test_tool");
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_global_registry() {
        GLOBAL_CONFIG_REGISTRY.configure("global_test", &[ToolConfig::desc("Global config")]);
        assert!(GLOBAL_CONFIG_REGISTRY.has_config("global_test"));

        GLOBAL_CONFIG_REGISTRY.clear("global_test");
        assert!(!GLOBAL_CONFIG_REGISTRY.has_config("global_test"));
    }

    // ---- T-002: priority table unit tests ----

    #[test]
    fn test_priority_order_array_matches_expected_ordering() {
        // The macro and the docs both depend on this exact order.
        assert_eq!(
            CONFIG_PRIORITY_ORDER,
            [
                ConfigLayer::ToolAttrDesc,
                ConfigLayer::DocComment,
                ConfigLayer::TokitaiConfig,
                ConfigLayer::Default,
            ]
        );
    }

    #[test]
    fn test_priority_table_md_contains_every_layer_label() {
        let table = config_priority_table_md();
        for layer in &CONFIG_PRIORITY_ORDER {
            let needle = layer.label();
            assert!(
                table.iter().any(|row| row.contains(needle)),
                "rendered table is missing label {:?}: {:?}",
                needle,
                table,
            );
        }
        assert_eq!(table.len(), CONFIG_PRIORITY_ORDER.len());
    }

    #[test]
    fn test_can_override_compile_time_attr_desc_is_locked() {
        // T-002 acceptance: `tokitai!` may NOT override an explicit
        // `#[tool(desc = "...")]`.
        assert!(!can_override(
            ConfigLayer::ToolAttrDesc.priority(),
            ConfigLayer::TokitaiConfig,
        ));
    }

    #[test]
    fn test_can_override_doc_comment_beats_runtime() {
        // Doc comments are higher priority than `tokitai!` config, so
        // the runtime cannot replace them either. This keeps the
        // priority order monotonic.
        assert!(!can_override(
            ConfigLayer::DocComment.priority(),
            ConfigLayer::TokitaiConfig,
        ));
    }

    #[test]
    fn test_can_override_default_is_always_open() {
        // Only the synthesized default falls below the runtime layer.
        assert!(can_override(
            ConfigLayer::Default.priority(),
            ConfigLayer::TokitaiConfig,
        ));
    }

    #[test]
    fn test_config_layer_priority_is_strictly_monotonic() {
        // The const table must be monotonic non-decreasing.
        for w in CONFIG_PRIORITY_ORDER.windows(2) {
            assert!(
                w[0].priority() < w[1].priority(),
                "{:?} ({}) should outrank {:?} ({})",
                w[0],
                w[0].priority(),
                w[1],
                w[1].priority(),
            );
        }
    }
}
