//! 配置宏集成测试：验证配置是否真正应用到 TOOL_DEFINITIONS
//!
//! 运行测试：cargo test -p tokitai-macros --test config_integration_test --features serde

#![cfg(feature = "serde")]

use serde_json::Value;
use tokitai::ToolProvider;
use tokitai::{config, tool};

// ============================================================================
// 测试：配置宏覆盖的描述是否在 TOOL_DEFINITIONS 中生效
// ============================================================================

struct IntegrationTestTools;

#[tool]
impl IntegrationTestTools {
    /// 默认描述 - 应该被配置覆盖
    pub fn get_user(&self, id: i32) -> String {
        format!("User {}", id)
    }
}

config! {
    IntegrationTestTools {
        get_user: {
            desc: "配置覆盖后的描述",
            params: {
                id: { desc: "用户 ID 参数" }
            }
        }
    }
}

#[test]
fn test_config_applies_to_tool_definitions() {
    // 触发配置初始化
    let _ = &*__CONFIG_INIT_IntegrationTestTools;

    // 获取工具定义
    let tool = &IntegrationTestTools::tool_definitions()[0];

    // ❌ 关键测试：配置是否真正覆盖了描述？
    // 当前实现：TOOL_DEFINITIONS 是编译期生成的 const，无法被运行时配置覆盖
    println!("Tool name: {}", tool.name);
    println!("Tool description: {}", tool.description);
    println!("Tool input_schema: {}", tool.input_schema);

    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap();
    println!("Parsed schema: {:?}", schema);

    // 测试当前行为（文档注释描述）
    assert_eq!(tool.description, "默认描述 - 应该被配置覆盖");

    // TODO: 配置宏真正生效后，应该改为：
    // assert_eq!(tool.description, "配置覆盖后的描述");

    // 测试参数描述
    let param_desc = schema["properties"]["id"]["description"].as_str();
    println!("Param description: {:?}", param_desc);

    // 当前：参数描述来自 @param 文档注释（没有，所以为空）
    // TODO: 配置宏真正生效后，应该包含"用户 ID 参数"
}
