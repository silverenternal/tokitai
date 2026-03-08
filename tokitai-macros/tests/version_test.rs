//! 测试 version、deprecated_since、remove_in、replaced_by 属性

use tokitai::tool;

#[tool]
pub struct VersionTools;

#[tool]
impl VersionTools {
    /// 旧方法（已弃用）
    #[tool(
        version = "0.1.0",
        deprecated = true,
        deprecated_since = "0.3.0",
        remove_in = "1.0.0",
        replaced_by = "new_method"
    )]
    pub fn old_method(&self) -> Result<String, tokitai::ToolError> {
        Ok("旧方法".to_string())
    }

    /// 新方法
    #[tool(version = "0.3.0")]
    pub fn new_method(&self) -> Result<String, tokitai::ToolError> {
        Ok("新方法".to_string())
    }

    /// 另一个旧方法（已弃用但未指定替代者）
    #[tool(
        version = "0.1.0",
        deprecated = true,
        deprecated_since = "0.2.0",
        remove_in = "0.5.0",
        allow = ["deprecated_missing_replaced_by"]
    )]
    pub fn deprecated_without_replaced_by(&self) -> Result<String, tokitai::ToolError> {
        Ok("已弃用但未指定替代者".to_string())
    }
}

#[test]
fn test_version_in_tool_definition() {
    let tools = VersionTools::TOOL_DEFINITIONS;

    let old_tool = tools.iter().find(|t| t.name == "old_method").unwrap();
    let new_tool = tools.iter().find(|t| t.name == "new_method").unwrap();

    // 验证旧方法的版本信息
    assert_eq!(old_tool.version, Some("0.1.0"));
    assert_eq!(old_tool.deprecated_since, Some("0.3.0"));
    assert_eq!(old_tool.remove_in, Some("1.0.0"));
    assert_eq!(old_tool.replaced_by, Some("new_method"));

    // 验证新方法的版本信息
    assert_eq!(new_tool.version, Some("0.3.0"));
    assert_eq!(new_tool.deprecated_since, None);
    assert_eq!(new_tool.remove_in, None);
    assert_eq!(new_tool.replaced_by, None);
}

#[test]
fn test_deprecated_without_replaced_by() {
    let tools = VersionTools::TOOL_DEFINITIONS;

    let deprecated_tool = tools
        .iter()
        .find(|t| t.name == "deprecated_without_replaced_by")
        .unwrap();

    // 验证弃用但未指定替代者的方法
    assert_eq!(deprecated_tool.version, Some("0.1.0"));
    assert_eq!(deprecated_tool.deprecated_since, Some("0.2.0"));
    assert_eq!(deprecated_tool.remove_in, Some("0.5.0"));
    // 注意：当未指定 replaced_by 时，宏会生成空字符串
    assert_eq!(deprecated_tool.replaced_by, Some(""));
}

#[test]
fn test_all_version_fields() {
    let tools = VersionTools::TOOL_DEFINITIONS;

    // 验证所有工具都有版本信息
    assert_eq!(tools.len(), 3);

    for tool in tools {
        // 所有工具都应该有 version 字段（即使为 None）
        // 已标记 deprecated 的工具应该有 deprecated_since 和 remove_in
        if tool.deprecated_since.is_some() {
            assert!(tool.remove_in.is_some());
        }
    }
}
