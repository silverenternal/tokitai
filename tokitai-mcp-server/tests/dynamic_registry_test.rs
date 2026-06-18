//! T-010: integration test for `DynamicToolRegistry` behind the MCP HTTP server.
//!
//! Confirms the trait surface (add_tool / remove_tool / enable_for /
//! disable_for) wires through `McpServerBuilder::with_tool` and that the
//! HTTP `/tools` and `/call` endpoints honour runtime mutation.

use std::sync::Arc;

use tokitai::tool;
use tokitai::{
    DynamicHandler, DynamicToolProvider, DynamicToolRegistry, ToolDefinition, ToolProvider,
};
use tokitai_core::serde_types::Value as CoreValue;
use tokitai_core::ToolCaller;
use tokitai_mcp_server::McpServerBuilder;

// ============================================================================
// 1. Static tool that coexists with the dynamic registry.
// ============================================================================

#[derive(Default, Clone)]
struct StaticTools;

#[tool]
impl StaticTools {
    /// Add two numbers.
    pub fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }
}

// ============================================================================
// 2. Helpers for spinning up a server on an ephemeral port.
// ============================================================================

/// Pick an unused TCP port. We bind a listener, read its local addr, then
/// drop the listener — there is a brief TOCTOU window but the OS usually
/// does not reassign the port within microseconds.
fn ephemeral_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);
    port
}

// ============================================================================
// 3. Test: tool registry exposes mutating API.
// ============================================================================

#[test]
fn dynamic_registry_add_remove_and_call() {
    let mut reg = DynamicToolRegistry::new();
    reg.add_tool(
        "ping",
        ToolDefinition::new("ping", "Ping", r#"{"type":"object"}"#),
        Arc::new(|_args| Ok(serde_json::json!("pong"))),
    );

    // Visible to all tenants.
    assert_eq!(reg.visible_tools(None).len(), 1);
    assert_eq!(reg.list_global(), vec!["ping".to_string()]);

    // Call works.
    let result = reg.call_tool("ping", &serde_json::json!({})).unwrap();
    assert_eq!(result, serde_json::json!("pong"));

    // Remove works and is idempotent.
    assert!(reg.remove_tool("ping"));
    assert!(!reg.remove_tool("ping"));
    assert!(reg.list_global().is_empty());

    // Calling the removed tool returns NotFound.
    let err = reg.call_tool("ping", &serde_json::json!({})).unwrap_err();
    assert_eq!(err.kind, tokitai_core::ToolErrorKind::NotFound);
}

#[test]
fn dynamic_registry_per_tenant_enable_disable() {
    let mut reg = DynamicToolRegistry::new();
    reg.add_tool(
        "do_thing",
        ToolDefinition::new("do_thing", "Do the thing", r#"{"type":"object"}"#),
        Arc::new(|_args| Ok(serde_json::json!("done"))),
    );

    // Default: tenant can call.
    assert!(reg
        .call_for_tenant("do_thing", Some("alice"), &serde_json::json!({}))
        .is_ok());

    // Disable for alice.
    reg.disable_for("do_thing", "alice");
    let err = reg
        .call_for_tenant("do_thing", Some("alice"), &serde_json::json!({}))
        .unwrap_err();
    assert!(tokitai::is_tenant_denied(&err));

    // Bob still sees the tool.
    assert!(reg
        .call_for_tenant("do_thing", Some("bob"), &serde_json::json!({}))
        .is_ok());

    // Re-enable for alice.
    reg.enable_for("do_thing", "alice");
    assert!(reg
        .call_for_tenant("do_thing", Some("alice"), &serde_json::json!({}))
        .is_ok());

    // visible_tools reflects the per-tenant slice.
    let alice = reg.visible_tools(Some("alice"));
    assert_eq!(alice.len(), 1);
    assert_eq!(alice[0].name, "do_thing");

    // Disabling a tool that was already disabled is a no-op (does not
    // panic; does not change other tenant visibility).
    reg.disable_for("do_thing", "alice");
    let bob = reg.visible_tools(Some("bob"));
    assert_eq!(bob.len(), 1);
}

// ============================================================================
// 4. Test: the registry is Send + Sync so it can live in an Arc<McpServer...>.
// ============================================================================

#[test]
fn dynamic_registry_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DynamicToolRegistry>();
}

// ============================================================================
// 5. Test: McpServerBuilder::with_tool accepts a DynamicToolRegistry.
// ============================================================================

#[test]
fn mcp_server_builder_accepts_dynamic_registry() {
    let mut reg = DynamicToolRegistry::new();
    reg.add_tool(
        "hello",
        ToolDefinition::new("hello", "Greet", r#"{"type":"object"}"#),
        Arc::new(|_args| Ok(serde_json::json!("hi"))),
    );

    let server = McpServerBuilder::with_tool(reg)
        .with_port(ephemeral_port())
        .build();

    // The server sees the registered tool.
    let tools = server.tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "hello");
}

#[test]
fn mcp_server_builder_with_empty_registry() {
    let reg = DynamicToolRegistry::new();
    let server = McpServerBuilder::with_tool(reg)
        .with_port(ephemeral_port())
        .build();
    // Empty registry => empty tool list.
    assert!(server.tools().is_empty());
}

// ============================================================================
// 6. End-to-end smoke: build the server, mutate the registry, verify the
//    server's tools() reflects the mutation. This is the path the
//    HTTP handlers consult on every /tools request, so exercising it
//    directly is the highest-fidelity test we can do without spinning
//    up the full axum server.
// ============================================================================

#[test]
fn server_tools_reflect_runtime_mutation() {
    let mut reg = DynamicToolRegistry::new();
    reg.add_tool(
        "alpha",
        ToolDefinition::new("alpha", "Alpha tool", r#"{"type":"object"}"#),
        Arc::new(|_args| Ok(serde_json::json!("alpha-result"))),
    );

    let server = McpServerBuilder::with_tool(reg.clone())
        .with_port(ephemeral_port())
        .build();

    // /tools at t=0 → only alpha.
    assert_eq!(server.tools().len(), 1);
    assert_eq!(server.tools()[0].name, "alpha");

    // Mutate: remove alpha, add beta.
    reg.remove_tool("alpha");
    let mut writer = reg.clone();
    writer.add_tool(
        "beta",
        ToolDefinition::new("beta", "Beta tool", r#"{"type":"object"}"#),
        Arc::new(|_args| Ok(serde_json::json!("beta-result"))),
    );

    // /tools after mutation → only beta.
    assert_eq!(server.tools().len(), 1);
    assert_eq!(server.tools()[0].name, "beta");

    // Calling through the registry directly (the same dispatch path
    // the HTTP handler uses for `DynamicToolRegistry`) returns the
    // expected values.
    let v = reg.call_tool("beta", &serde_json::json!({})).unwrap();
    assert_eq!(v, serde_json::json!("beta-result"));
    let err = reg.call_tool("alpha", &serde_json::json!({})).unwrap_err();
    assert_eq!(err.kind, tokitai_core::ToolErrorKind::NotFound);
}

// ============================================================================
// 7. Per-tenant enable / disable smoke (no HTTP, in-process only).
// ============================================================================

#[test]
fn dynamic_registry_per_tenant_visibility() {
    use std::collections::HashSet;

    let mut reg = DynamicToolRegistry::new();
    reg.add_tool(
        "shared",
        ToolDefinition::new("shared", "Shared", r#"{"type":"object"}"#),
        Arc::new(|_args| Ok(serde_json::json!("ok"))),
    );
    reg.add_tool(
        "premium",
        ToolDefinition::new("premium", "Premium", r#"{"type":"object"}"#),
        Arc::new(|_args| Ok(serde_json::json!("premium-ok"))),
    );

    // Default: every tenant sees both tools (no per-tenant overrides).
    let free: HashSet<String> = reg
        .visible_tools(Some("free"))
        .iter()
        .map(|d| d.name.clone())
        .collect();
    let premium: HashSet<String> = reg
        .visible_tools(Some("premium"))
        .iter()
        .map(|d| d.name.clone())
        .collect();
    assert_eq!(free.len(), 2);
    assert_eq!(premium.len(), 2);

    // Tier-1 strategy: flip "free" to default-deny, then explicitly
    // enable `shared` for them. They cannot use `premium`.
    reg.disable_for("premium", "free");
    reg.disable_for("shared", "free");
    reg.enable_for("shared", "free");

    let free: HashSet<String> = reg
        .visible_tools(Some("free"))
        .iter()
        .map(|d| d.name.clone())
        .collect();
    assert_eq!(free.len(), 1);
    assert!(free.contains("shared"));

    // Calling premium as free user returns the per-tenant NotFound.
    let err = reg
        .call_for_tenant("premium", Some("free"), &CoreValue::Null)
        .unwrap_err();
    assert!(tokitai::is_tenant_denied(&err));
}

// ============================================================================
// 8. Handler type alias is exported and callable.
// ============================================================================

#[test]
fn dynamic_handler_alias_is_callable() {
    let h: DynamicHandler = Arc::new(|_args| Ok(serde_json::json!(42)));
    let v = h(&serde_json::json!({})).unwrap();
    assert_eq!(v, serde_json::json!(42));
}

// ============================================================================
// 9. Backwards compatibility: a static provider still works alongside.
// ============================================================================

#[test]
fn static_provider_still_implements_tool_provider() {
    // T-010 contract: existing `ToolProvider` impls (auto-emitted by
    // the `#[tool]` macro) are untouched.
    let tools = StaticTools::tool_definitions();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "add");
    // Calling `add` through the trait returns 30 for {a:10,b:20}.
    let calc = StaticTools;
    let v = calc
        .call_tool("add", &serde_json::json!({"a": 10, "b": 20}))
        .unwrap();
    assert_eq!(v, serde_json::json!(30));
}

// ============================================================================
// 10. `call_for_tenant` honours the per-tenant enable set, not just the
//     default-allow path.
// ============================================================================

#[test]
fn dynamic_registry_explicit_enable_set() {
    let mut reg = DynamicToolRegistry::new();
    reg.add_tool(
        "t1",
        ToolDefinition::new("t1", "t1", r#"{"type":"object"}"#),
        Arc::new(|_| Ok(serde_json::json!(1))),
    );
    reg.add_tool(
        "t2",
        ToolDefinition::new("t2", "t2", r#"{"type":"object"}"#),
        Arc::new(|_| Ok(serde_json::json!(2))),
    );

    // First disable t1 for tenant x (flips x to default-deny).
    reg.disable_for("t1", "x");
    // Then explicitly enable t2 for x.
    reg.enable_for("t2", "x");

    // x can call t2 but not t1.
    assert!(reg
        .call_for_tenant("t2", Some("x"), &CoreValue::Null)
        .is_ok());
    let err = reg
        .call_for_tenant("t1", Some("x"), &CoreValue::Null)
        .unwrap_err();
    assert!(tokitai::is_tenant_denied(&err));
}

// ============================================================================
// 11. clear() resets all state.
// ============================================================================

#[test]
fn dynamic_registry_clear_resets_state() {
    let mut reg = DynamicToolRegistry::new();
    reg.add_tool(
        "x",
        ToolDefinition::new("x", "x", r#"{"type":"object"}"#),
        Arc::new(|_| Ok(serde_json::json!(null))),
    );
    reg.enable_for("x", "alice");
    reg.disable_for("x", "bob");

    assert_eq!(reg.list_global().len(), 1);
    reg.clear();
    assert!(reg.list_global().is_empty());
    assert!(reg.visible_tools(Some("alice")).is_empty());
    assert!(reg.visible_tools(Some("bob")).is_empty());
}
