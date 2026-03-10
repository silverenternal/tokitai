//! 配置宏 UI 测试：验证 tokitai! 配置宏功能
//!
//! 运行测试：cargo test -p tokitai-macros --test config_macro_test --features serde

#![cfg(feature = "serde")]

use tokitai::ToolCaller;
use tokitai::{config, tool, ToolConfig};

// ============================================================================
// 测试 1: 配置宏覆盖工具描述
// ============================================================================

#[derive(Default)]
struct ConfigTestTools;

#[tool]
impl ConfigTestTools {
    /// 默认描述
    pub fn get_user(&self, id: i32) -> String {
        format!("User {}", id)
    }
}

config! {
    ConfigTestTools {
        get_user: {
            desc: "配置覆盖后的描述",
            params: {
                id: { desc: "用户 ID 参数" }
            }
        }
    }
}

#[test]
fn test_config_macro_desc_override() {
    // 触发配置初始化（访问静态变量）
    let _ = &*__CONFIG_INIT_ConfigTestTools;

    // 验证配置已注册
    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("get_user"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("get_user");
    assert!(!configs.is_empty());

    // 验证包含 Desc 配置
    let has_desc = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "配置覆盖后的描述"));
    assert!(has_desc, "应该包含 Desc 配置");

    // 验证包含 ParamDesc 配置
    let has_param_desc = configs.iter().any(|c| matches!(c, ToolConfig::ParamDesc { name, desc } if name == "id" && desc == "用户 ID 参数"));
    assert!(has_param_desc, "应该包含 ParamDesc 配置");
}

// ============================================================================
// 测试 2: 配置宏添加 tags
// ============================================================================

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
            desc: "搜索功能",
            tags: ["search", "utility"]
        }
    }
}

#[test]
fn test_config_macro_tags() {
    // 触发配置初始化
    let _ = &*__CONFIG_INIT_ConfigTagsTools;

    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("search"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("search");

    // 验证包含 Tags 配置
    let has_tags = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Tags(tags) if tags.contains(&"search".to_string())));
    assert!(has_tags, "应该包含 Tags 配置");
}

// ============================================================================
// 测试 3: 配置宏添加参数示例
// ============================================================================

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
            desc: "问候功能",
            params: {
                name: {
                    desc: "姓名",
                    example: "张三"
                }
            }
        }
    }
}

#[test]
fn test_config_macro_param_example() {
    // 触发配置初始化
    let _ = &*__CONFIG_INIT_ConfigExampleTools;

    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("greet"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("greet");

    // 验证包含 ParamExample 配置
    let has_example = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::ParamExample { name, .. } if name == "name"));
    assert!(has_example, "应该包含 ParamExample 配置");
}

// ============================================================================
// 测试 4: 配置宏多个方法配置
// ============================================================================

struct MultiMethodTools;

#[tool]
impl MultiMethodTools {
    /// 方法 1 默认描述
    pub fn method1(&self, a: i32) -> i32 {
        a
    }

    /// 方法 2 默认描述
    pub fn method2(&self, b: String) -> String {
        b
    }
}

config! {
    MultiMethodTools {
        method1: {
            desc: "方法 1 配置描述",
            params: {
                a: { desc: "参数 a" }
            }
        },
        method2: {
            desc: "方法 2 配置描述",
            tags: ["custom"]
        }
    }
}

#[test]
fn test_config_macro_multiple_methods() {
    // 触发配置初始化
    let _ = &*__CONFIG_INIT_MultiMethodTools;

    // 验证两个方法都有配置
    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("method1"));
    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("method2"));

    let configs1 = tokitai::GLOBAL_CONFIG_REGISTRY.get("method1");
    let configs2 = tokitai::GLOBAL_CONFIG_REGISTRY.get("method2");

    // 验证方法 1 配置
    let has_desc1 = configs1
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "方法 1 配置描述"));
    assert!(has_desc1, "方法 1 应该包含 Desc 配置");

    // 验证方法 2 配置
    let has_desc2 = configs2
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "方法 2 配置描述"));
    assert!(has_desc2, "方法 2 应该包含 Desc 配置");

    let has_tags2 = configs2
        .iter()
        .any(|c| matches!(c, ToolConfig::Tags(tags) if tags.contains(&"custom".to_string())));
    assert!(has_tags2, "方法 2 应该包含 Tags 配置");
}
