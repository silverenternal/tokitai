//! 集成测试：测试 ToolProvider 与工具定义的集成

use tokitai::ToolDefinition;

#[test]
fn test_tool_definition_structure() {
    // 验证 ToolDefinition 的基本结构
    let def = ToolDefinition::new(
        "test_tool",
        "A test tool",
        r#"{"type":"object","properties":{},"required":[]}"#,
    );
    assert_eq!(def.name, "test_tool");
    assert_eq!(def.description, "A test tool");
}

#[test]
fn test_tool_definition_with_builder() {
    let def = ToolDefinition::new(
        "test",
        "Test description",
        r#"{"type":"object","properties":{},"required":[]}"#,
    )
    .with_version("1.0.0".to_string());

    assert_eq!(def.name, "test");
    assert_eq!(def.version, Some("1.0.0".to_string()));
}

#[test]
fn test_tool_definition_to_json() {
    let def = ToolDefinition::new(
        "json_test",
        "Test JSON serialization",
        r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#,
    )
    .with_version("2.0.0".to_string());

    let json = def.to_json().unwrap();
    assert!(json.contains("json_test"));
    assert!(json.contains("2.0.0"));
}

#[test]
fn test_tool_config_registry() {
    use tokitai_core::config::{ToolConfig, ToolConfigRegistry};

    let registry = ToolConfigRegistry::new();

    // 配置一个工具
    let config = ToolConfig::desc("Updated description".to_string());

    registry.configure("my_tool", &[config]);

    // 验证配置被保存
    let configs = registry.get("my_tool");
    assert_eq!(configs.len(), 1);

    // 验证 has_config
    assert!(registry.has_config("my_tool"));
    assert!(!registry.has_config("nonexistent_tool"));
}

#[test]
fn test_tool_config_clear() {
    use tokitai_core::config::{ToolConfig, ToolConfigRegistry};

    let registry = ToolConfigRegistry::new();

    // 添加配置
    registry.configure("tool1", &[ToolConfig::desc("desc1".to_string())]);
    registry.configure("tool2", &[ToolConfig::desc("desc2".to_string())]);

    assert!(registry.has_config("tool1"));
    assert!(registry.has_config("tool2"));

    // 清除所有
    registry.clear_all();

    assert!(!registry.has_config("tool1"));
    assert!(!registry.has_config("tool2"));
}

#[test]
fn test_global_registry_access() {
    use tokitai::GLOBAL_CONFIG_REGISTRY;

    // 验证全局注册表可以访问
    GLOBAL_CONFIG_REGISTRY.configure("global_test", &[]);
    let configs = GLOBAL_CONFIG_REGISTRY.get("global_test");
    assert!(configs.is_empty()); // 没有配置，返回空向量
}

#[test]
fn test_input_schema_pretty() {
    let def = ToolDefinition::new(
        "pretty_test",
        "Test pretty print",
        r#"{"type":"object","properties":{"age":{"type":"integer"}}}"#,
    );

    let pretty = def.input_schema_pretty().unwrap();
    assert!(pretty.contains("age"));
    assert!(pretty.contains("integer"));
}
