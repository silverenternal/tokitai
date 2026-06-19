//! T-010: Dynamic / runtime-mutable tool registry.
//!
//! The compile-time [`crate::ToolProvider`] trait returns `&'static [ToolDefinition]`,
//! which by definition cannot grow or shrink at runtime. That rigidity is
//! the right default for the macro hot path, but it blocks legitimate
//! multi-tenant use cases: per-tenant allow-lists, hot-reload of tool
//! plugins, AB-test arms that expose a different toolset to each user.
//!
//! This module adds a separate trait, [`DynamicToolProvider`], with a
//! concrete [`DynamicToolRegistry`] backing store. The macro-generated
//! providers do **not** implement it (that would defeat the compile-time
//! guarantee); instead, [`DynamicToolRegistry`] wraps a heterogeneous
//! `Vec<Box<dyn ToolCallerDyn>>` and exposes `add_tool`, `remove_tool`,
//! `enable_for`, and `disable_for` so callers can mutate the visible
//! toolset at runtime.
//!
//! The trait is **additive** — existing `ToolProvider` impls continue to
//! work unchanged. The struct is `Send + Sync` so it can be shared
//! between axum handlers in the MCP server.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use crate::serde_types::Value;
use crate::{ToolDefinition, ToolError, ToolErrorKind};

/// T-010: trait for runtime-mutable tool registries.
///
/// Distinct from the compile-time [`crate::ToolProvider`] trait. Existing macro-
/// generated providers do not implement this — the macro's whole value
/// proposition is "schema is fixed at compile time." [`DynamicToolProvider`]
/// is opt-in: callers who need mutability build a
/// [`DynamicToolRegistry`] (or implement this trait themselves) and
/// serve tools through it.
///
/// The trait surface is deliberately small and matches the pain points
/// in PP-B1:
///
/// * `add_tool` / `remove_tool` — global registration, takes effect for
///   every caller.
/// * `enable_for` / `disable_for` — per-tenant overrides layered on top
///   of the global registry.
///
/// Per-call dispatch (`call_tool`) goes through the existing
/// [`crate::ToolCaller`] trait; [`DynamicToolRegistry`] implements both.
///
/// # Example
///
/// ```rust,ignore
/// use tokitai_core::{DynamicToolProvider, DynamicToolRegistry, ToolDefinition, ToolError};
/// use serde_json::json;
///
/// let mut reg = DynamicToolRegistry::new();
/// reg.add_tool(
///     "add",
///     ToolDefinition::new("add", "Add two numbers", r#"{"type":"object"}"#),
///     Arc::new(|_args| Ok(json!(3))),
/// );
/// let v = reg.call_tool("add", &json!({"a":1,"b":2})).unwrap();
/// assert_eq!(v, json!(3));
/// ```
///
/// T-010: handler signature for dynamically-registered tools.
///
/// The closure receives the JSON args object the caller supplied and
/// must return either the result or a [`ToolError`]. Implementations
/// can `Arc::new(|args| { ... })` any `Fn(&Value) -> Result<Value,
/// ToolError> + Send + Sync` closure.
pub type DynamicHandler = Arc<dyn Fn(&Value) -> Result<Value, ToolError> + Send + Sync>;

/// T-010: trait for runtime-mutable tool registries.
///
/// Distinct from the compile-time [`crate::ToolProvider`] trait.
/// Existing macro-generated providers do not implement this — the
/// macro's whole value proposition is "schema is fixed at compile
/// time." [`DynamicToolProvider`] is opt-in: callers who need
/// mutability build a [`DynamicToolRegistry`] (or implement this
/// trait themselves) and serve tools through it.
///
/// The trait surface is deliberately small and matches the pain
/// points in PP-B1:
///
/// * `add_tool` / `remove_tool` — global registration, takes effect
///   for every caller.
/// * `enable_for` / `disable_for` — per-tenant overrides layered on
///   top of the global registry.
///
/// Per-call dispatch (`call_tool`) goes through the existing
/// [`crate::ToolCaller`] trait; [`DynamicToolRegistry`] implements
/// both.
pub trait DynamicToolProvider {
    /// Register a tool under `name`. Replaces any existing registration
    /// under the same name. Returns the registration's [`ToolDefinition`]
    /// for inspection / chaining.
    ///
    /// `handler` is the closure that actually runs when the tool is
    /// invoked; it receives the JSON args object and must return either
    /// the result JSON or a [`ToolError`].
    fn add_tool(&mut self, name: &str, definition: ToolDefinition, handler: DynamicHandler);

    /// Remove a tool from the global registry. Returns `true` when the
    /// tool existed and was removed; `false` when no such tool was
    /// registered.
    fn remove_tool(&mut self, name: &str) -> bool;

    /// Enable `name` for a specific `tenant`. The tool must already be
    /// in the global registry (use [`DynamicToolProvider::add_tool`]
    /// first); this method only flips the per-tenant visibility flag.
    ///
    /// A tenant id is any `&str` the caller picks (user id, API key,
    /// session id, etc.); it is opaque to the registry.
    fn enable_for(&mut self, name: &str, tenant: &str);

    /// Disable `name` for a specific `tenant`. Has no effect on the
    /// global registry; other tenants keep their visibility.
    fn disable_for(&mut self, name: &str, tenant: &str);

    /// List the tools visible to `tenant` (or, when `tenant` is `None`,
    /// every globally-registered tool). Useful for diagnostics / REST
    /// `/tools` endpoints.
    fn visible_tools(&self, tenant: Option<&str>) -> Vec<ToolDefinition>;
}

/// T-010: concrete dynamic registry.
///
/// Thread-safe (`Arc<RwLock<...>>` internals) so it can be shared
/// between axum handlers in `tokitai-mcp-server` and a long-running
/// admin endpoint that adds/removes tools at runtime.
///
/// Tool definitions live in an `Inner` struct behind an `Arc<RwLock>`.
/// Per-tenant overrides are layered on top: `enable_for` /
/// `disable_for` mutate a `HashMap<tenant, HashSet<tool_name>>`. A
/// tool is visible to a tenant when:
///
/// 1. it is globally registered, AND
/// 2. the tenant has no entry in the overrides map (default-allow), OR
/// 3. the tenant's entry contains the tool name.
///
/// Note: when a tenant's first override is a `disable_for`, that flips
/// the default-allow to default-deny for that tenant. Callers wanting
/// to keep default-allow semantics should `enable_for` every global
/// tool after the first `disable_for`.
#[derive(Default, Clone)]
pub struct DynamicToolRegistry {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Default)]
struct Inner {
    /// Globally-registered tools (always visible unless a tenant
    /// override disables them).
    tools: HashMap<String, RegisteredTool>,
    /// Per-tenant allow/deny sets. `Some(set)` means "the tenant's
    /// visibility is `set`" — empty `set` = deny-all for that tenant.
    /// `None` means "use the global default (allow-all)".
    per_tenant: HashMap<String, HashSet<String>>,
}

struct RegisteredTool {
    definition: ToolDefinition,
    handler: DynamicHandler,
}

impl DynamicToolRegistry {
    /// Create an empty registry.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokitai_core::DynamicToolRegistry;
    ///
    /// let reg = DynamicToolRegistry::new();
    /// assert!(reg.list_global().is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// List the names of all globally-registered tools, in
    /// insertion order (HashMap ordering is unspecified, but the set
    /// itself is deterministic).
    pub fn list_global(&self) -> Vec<String> {
        self.inner
            .read()
            .map(|guard| guard.tools.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Return `true` if `name` is currently registered (globally).
    pub fn contains(&self, name: &str) -> bool {
        self.inner
            .read()
            .map(|guard| guard.tools.contains_key(name))
            .unwrap_or(false)
    }

    /// Return the [`ToolDefinition`] for a globally-registered tool,
    /// or `None` if it is not registered.
    pub fn definition(&self, name: &str) -> Option<ToolDefinition> {
        self.inner
            .read()
            .ok()
            .and_then(|guard| guard.tools.get(name).map(|t| t.definition.clone()))
    }

    /// Drop all globally-registered tools and per-tenant overrides.
    /// Useful for tests and for "reset to known state" admin flows.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.tools.clear();
            guard.per_tenant.clear();
        }
    }

    /// Invoke `name` honouring `tenant`'s visibility rules.
    /// Returns `ToolError::NotFound` when the tool is not visible to
    /// `tenant` (whether because it was never registered, or because
    /// the tenant's override set excludes it).
    pub fn call_for_tenant(
        &self,
        name: &str,
        tenant: Option<&str>,
        args: &Value,
    ) -> Result<Value, ToolError> {
        let guard = self
            .inner
            .read()
            .map_err(|e| ToolError::internal_error(format!("registry poisoned: {}", e)))?;
        let tool = guard
            .tools
            .get(name)
            .ok_or_else(|| ToolError::not_found(format!("tool `{}` is not registered", name)))?;
        if let Some(t) = tenant {
            if let Some(set) = guard.per_tenant.get(t) {
                if !set.contains(name) {
                    return Err(ToolError::not_found(format!(
                        "tool `{}` is not enabled for tenant `{}`",
                        name, t
                    )));
                }
            }
        }
        (tool.handler)(args)
    }
}

impl DynamicToolProvider for DynamicToolRegistry {
    fn add_tool(&mut self, name: &str, definition: ToolDefinition, handler: DynamicHandler) {
        // The trait signature takes `&mut self`, but our backing
        // store is `Arc<RwLock<...>>` so callers can share the
        // registry across threads. Deref to `&self` for the actual
        // mutation; `Arc::make_mut` would be cleaner but the lock
        // pattern is more uniform with the rest of the API.
        if let Ok(mut guard) = self.inner.write() {
            guard.tools.insert(
                name.to_string(),
                RegisteredTool {
                    definition,
                    handler,
                },
            );
        }
    }

    fn remove_tool(&mut self, name: &str) -> bool {
        if let Ok(mut guard) = self.inner.write() {
            // Also scrub the name from every tenant's allow-set so
            // we don't leak stale names into future visibility
            // queries. If a tenant's set becomes empty as a result,
            // drop the entry entirely so the tenant falls back to
            // default-allow semantics.
            for (tenant, set) in guard.per_tenant.iter_mut() {
                set.remove(name);
                let _ = tenant; // used by the entry-removal pass below
            }
            guard.per_tenant.retain(|_tenant, set| !set.is_empty());
            guard.tools.remove(name).is_some()
        } else {
            false
        }
    }

    fn enable_for(&mut self, name: &str, tenant: &str) {
        let Ok(mut guard) = self.inner.write() else {
            return;
        };
        // No-op when the tool isn't globally registered; matches the
        // documented contract that `enable_for` only flips visibility
        // for already-registered tools.
        if !guard.tools.contains_key(name) {
            return;
        }
        let entry = guard
            .per_tenant
            .entry(tenant.to_string())
            .or_insert_with(HashSet::new);
        entry.insert(name.to_string());
    }

    fn disable_for(&mut self, name: &str, tenant: &str) {
        let Ok(mut guard) = self.inner.write() else {
            return;
        };
        // Disabling a tool for a tenant implicitly flips the tenant
        // to "default-deny" semantics: we insert an empty allow-set
        // if the tenant doesn't have an override yet, then remove
        // the tool name. This is the only way to take a globally-
        // visible tool away from a tenant without affecting other
        // tenants.
        let entry = guard
            .per_tenant
            .entry(tenant.to_string())
            .or_insert_with(HashSet::new);
        entry.remove(name);
    }

    fn visible_tools(&self, tenant: Option<&str>) -> Vec<ToolDefinition> {
        let Ok(guard) = self.inner.read() else {
            return Vec::new();
        };
        match tenant {
            None => guard.tools.values().map(|t| t.definition.clone()).collect(),
            Some(t) => {
                if let Some(set) = guard.per_tenant.get(t) {
                    guard
                        .tools
                        .values()
                        .filter(|tool| set.contains(&tool.definition.name))
                        .map(|t| t.definition.clone())
                        .collect()
                } else {
                    // Tenant has no overrides: default-allow all
                    // globally-registered tools.
                    guard.tools.values().map(|t| t.definition.clone()).collect()
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// ToolProvider / ToolCaller integration so the registry plugs into existing
// transports (MCP HTTP server, stdio transport, etc.) without a separate
// wrapper.
// -----------------------------------------------------------------------------

impl crate::ToolProvider for DynamicToolRegistry {
    /// The compile-time static slice is empty by design — the
    /// registry is purely runtime. Servers that need the list call
    /// [`DynamicToolProvider::visible_tools`] instead.
    fn tool_definitions() -> &'static [ToolDefinition] {
        // T-010: We can't return a `&'static` slice to the runtime
        // HashMap without leaking memory. Returning the empty
        // `&'static` slice is the documented contract for
        // "toolset is dynamic; ask via the instance method." The
        // MCP server already special-cases `MultiToolProvider` for
        // exactly this reason.
        const EMPTY: &[ToolDefinition] = &[];
        EMPTY
    }
}

impl crate::ToolCaller for DynamicToolRegistry {
    fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
        // No tenant context at this layer — every dynamically-
        // registered tool is reachable when the caller has the
        // registry handle. Multi-tenant gating is layered on top
        // via [`DynamicToolRegistry::call_for_tenant`].
        self.call_for_tenant(name, None, args)
    }
}

/// T-023: dynamic registries inherit the trait's default
/// (empty) capability manifest. Operators who need a
/// per-tenant capability allowlist for a dynamic registry can
/// wrap it in a `McpServerBuilder::with_tool(registry)` and
/// configure the allowlist on the builder. The static
/// `#[tool]` macro path is the primary T-023 surface; the
/// dynamic path is left as a follow-up (the per-tenant gating
/// already provides a richer policy surface than the static
/// manifest, so collapsing the two would be a net loss).
impl crate::CapabilityManifestProvider for DynamicToolRegistry {}

/// T-010: per-tenant error variant for the case where a tool exists
/// globally but is gated off for the caller. Distinct from
/// [`ToolErrorKind::NotFound`] so callers can distinguish "I never
/// registered this tool" from "you can't use this tool" when
/// debugging per-tenant allow-list misconfigurations.
pub const TENANT_DENIED_KIND_HINT: &str = "tenant-denied";

/// Helper to extract the per-tenant `not_found` reason from a
/// [`ToolError`]. Returns `true` when the error is the per-tenant
/// gating flavour (i.e. the message starts with
/// `"tool \`X\` is not enabled for tenant \`Y\`"`).
pub fn is_tenant_denied(err: &ToolError) -> bool {
    err.kind == ToolErrorKind::NotFound && err.message.contains("is not enabled for tenant")
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolCaller;
    use crate::ToolProvider;
    use serde_json::json;

    fn add_handler(a: i64, b: i64) -> DynamicHandler {
        Arc::new(move |_args| Ok(json!(a + b)))
    }

    #[test]
    fn add_and_call() {
        let mut reg = DynamicToolRegistry::new();
        reg.add_tool(
            "add",
            ToolDefinition::new("add", "Add", r#"{"type":"object"}"#),
            add_handler(1, 2),
        );
        let v = reg.call_tool("add", &json!({})).unwrap();
        assert_eq!(v, json!(3));
        assert!(reg.contains("add"));
        assert_eq!(reg.list_global(), vec!["add".to_string()]);
    }

    #[test]
    fn remove_tool_returns_true_then_false() {
        let mut reg = DynamicToolRegistry::new();
        reg.add_tool(
            "x",
            ToolDefinition::new("x", "x", "{}"),
            Arc::new(|_| Ok(json!(null))),
        );
        assert!(reg.remove_tool("x"));
        assert!(!reg.remove_tool("x"));
    }

    #[test]
    fn enable_disable_for_tenant() {
        let mut reg = DynamicToolRegistry::new();
        reg.add_tool(
            "t",
            ToolDefinition::new("t", "t", "{}"),
            Arc::new(|_| Ok(json!("ok"))),
        );

        // Default: tenant A sees the tool.
        assert!(reg.call_for_tenant("t", Some("a"), &json!({})).is_ok());
        // disable_for("t", "a") flips A to default-deny.
        reg.disable_for("t", "a");
        let err = reg.call_for_tenant("t", Some("a"), &json!({})).unwrap_err();
        assert!(is_tenant_denied(&err));
        // Other tenant still sees the tool.
        assert!(reg.call_for_tenant("t", Some("b"), &json!({})).is_ok());
        // enable_for restores access.
        reg.enable_for("t", "a");
        assert!(reg.call_for_tenant("t", Some("a"), &json!({})).is_ok());
    }

    #[test]
    fn enable_for_unknown_tool_is_noop() {
        let mut reg = DynamicToolRegistry::new();
        reg.enable_for("nope", "a");
        // No crash, no entry created.
        assert!(!reg.contains("nope"));
    }

    #[test]
    fn remove_tool_clears_per_tenant_set() {
        let mut reg = DynamicToolRegistry::new();
        reg.add_tool(
            "t",
            ToolDefinition::new("t", "t", "{}"),
            Arc::new(|_| Ok(json!(null))),
        );
        reg.enable_for("t", "a");
        assert!(reg.remove_tool("t"));
        // After removal, the per-tenant set should not still mention
        // `t`. We probe by re-adding and ensuring visibility resets.
        reg.add_tool(
            "t",
            ToolDefinition::new("t", "t", "{}"),
            Arc::new(|_| Ok(json!(null))),
        );
        // Tenant A's per-tenant set was scrubbed, so they see `t`
        // again via default-allow semantics.
        assert!(reg.call_for_tenant("t", Some("a"), &json!({})).is_ok());
    }

    #[test]
    fn visible_tools_filters_by_tenant() {
        let mut reg = DynamicToolRegistry::new();
        reg.add_tool(
            "a",
            ToolDefinition::new("a", "a", "{}"),
            Arc::new(|_| Ok(json!(null))),
        );
        reg.add_tool(
            "b",
            ToolDefinition::new("b", "b", "{}"),
            Arc::new(|_| Ok(json!(null))),
        );
        let all = reg.visible_tools(None);
        assert_eq!(all.len(), 2);

        reg.disable_for("a", "alice");
        reg.disable_for("b", "alice");
        let alice = reg.visible_tools(Some("alice"));
        assert!(alice.is_empty());

        reg.enable_for("b", "alice");
        let alice = reg.visible_tools(Some("alice"));
        assert_eq!(alice.len(), 1);
        assert_eq!(alice[0].name, "b");
    }

    #[test]
    fn tool_provider_static_slice_is_empty() {
        // T-010 contract: the dynamic registry's static slice is
        // empty by design. Server code must call the instance
        // method to discover tools.
        assert!(DynamicToolRegistry::tool_definitions().is_empty());
    }

    #[test]
    fn clear_resets_state() {
        let mut reg = DynamicToolRegistry::new();
        reg.add_tool(
            "x",
            ToolDefinition::new("x", "x", "{}"),
            Arc::new(|_| Ok(json!(null))),
        );
        reg.enable_for("x", "a");
        reg.clear();
        assert!(reg.list_global().is_empty());
        assert!(reg.visible_tools(Some("a")).is_empty());
    }

    #[test]
    fn missing_tool_returns_not_found() {
        let reg = DynamicToolRegistry::new();
        let err = reg.call_tool("ghost", &json!({})).unwrap_err();
        assert_eq!(err.kind, ToolErrorKind::NotFound);
    }

    #[test]
    fn handler_error_propagates() {
        let mut reg = DynamicToolRegistry::new();
        reg.add_tool(
            "boom",
            ToolDefinition::new("boom", "boom", "{}"),
            Arc::new(|_| Err(ToolError::internal_error("kaboom"))),
        );
        let err = reg.call_tool("boom", &json!({})).unwrap_err();
        assert_eq!(err.kind, ToolErrorKind::InternalError);
        assert!(err.message.contains("kaboom"));
    }

    #[test]
    fn registry_is_send_sync() {
        // Compile-time assertion: DynamicToolRegistry must be
        // Send + Sync so it can live behind an Arc<McpServerWithProvider<T>>.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DynamicToolRegistry>();
    }
}
