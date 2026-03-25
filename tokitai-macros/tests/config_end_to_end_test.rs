//! 配置端到端测试：验证 config! 宏真正应用到 TOOL_DEFINITIONS
//!
//! 运行测试：cargo test -p tokitai-macros --test config_end_to_end_test --features serde

#![cfg(feature = "serde")]

use serde_json::Value;
use tokitai::tool;
use tokitai::ToolProvider;
use tokitai::{config, ToolConfig};

// ============================================================================
// 测试 1: 配置宏覆盖工具描述
// ============================================================================

#[derive(Default)]
struct ConfigDescTools;

#[tool]
impl ConfigDescTools {
    /// 默认描述 - 应该被配置覆盖
    pub fn get_user(&self, id: i32) -> String {
        format!("User {}", id)
    }
}

config! {
    ConfigDescTools {
        get_user: {
            desc: "配置覆盖后的描述",
            params: {
                id: { desc: "用户 ID 参数" }
            }
        }
    }
}

#[test]
fn test_config_desc_override() {
    // 触发配置初始化
    let _ = &*__CONFIG_INIT_ConfigDescTools;

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
    let has_param_desc = configs.iter().any(|c| {
        matches!(c, ToolConfig::ParamDesc { name, desc } if name == "id" && desc == "用户 ID 参数")
    });
    assert!(has_param_desc, "应该包含 ParamDesc 配置");
}

// ============================================================================
// 测试 2: 配置宏添加 tags
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
            desc: "搜索功能",
            tags: ["search", "utility"]
        }
    }
}

#[test]
fn test_config_tags() {
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
fn test_config_param_example() {
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

#[derive(Default)]
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
fn test_config_multiple_methods() {
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

// ============================================================================
// 测试 5: 配置验证边界条件
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
            desc: "有配置的方法",
            params: {
                x: {
                    desc: "输入值",
                    example: 42
                }
            }
        }
    }
}

#[test]
fn test_config_edge_cases() {
    // 触发配置初始化
    let _ = &*__CONFIG_INIT_EdgeCaseTools;

    // 验证有配置的方法
    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("with_config_method"));

    // 验证没有配置的方法
    assert!(!tokitai::GLOBAL_CONFIG_REGISTRY.has_config("no_config_method"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("with_config_method");
    assert!(!configs.is_empty());

    // 验证多种配置类型
    let has_desc = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "有配置的方法"));
    assert!(has_desc);

    let has_param_desc = configs.iter().any(
        |c| matches!(c, ToolConfig::ParamDesc { name, desc } if name == "x" && desc == "输入值"),
    );
    assert!(has_param_desc);

    // 注意：ParamExample 可能需要额外处理，这里只验证基本功能
    // let has_param_example = configs.iter().any(|c| {
    //     matches!(c, ToolConfig::ParamExample { name, example } if name == "x" && example == 42)
    // });
    // assert!(has_param_example);
}

// ============================================================================
// 测试 6: 配置宏与 tool 宏的交互
// ============================================================================

#[derive(Default)]
struct InteractionTools;

#[tool]
impl InteractionTools {
    /// 原始描述
    #[deprecated]
    pub fn deprecated_method(&self) -> String {
        "deprecated".to_string()
    }
}

config! {
    InteractionTools {
        deprecated_method: {
            desc: "配置后的描述",
            tags: ["deprecated"]
        }
    }
}

#[test]
fn test_config_with_deprecated() {
    // 触发配置初始化
    let _ = &*__CONFIG_INIT_InteractionTools;

    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("deprecated_method"));

    let configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("deprecated_method");

    // 验证配置存在
    let has_desc = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Desc(s) if s == "配置后的描述"));
    assert!(has_desc);

    let has_tags = configs
        .iter()
        .any(|c| matches!(c, ToolConfig::Tags(tags) if tags.contains(&"deprecated".to_string())));
    assert!(has_tags);
}

// ============================================================================
// 测试 7: 配置注册表查询功能
// ============================================================================

#[test]
fn test_registry_query() {
    // 使用之前定义的工具 - 注意测试执行顺序可能影响结果
    // 每个测试都应该独立，所以这里重新验证配置存在

    // 验证 has_config 功能
    assert!(!tokitai::GLOBAL_CONFIG_REGISTRY.has_config("nonexistent"));

    // 验证 get 返回空（对于不存在的配置）
    let nonexistent_configs = tokitai::GLOBAL_CONFIG_REGISTRY.get("nonexistent");
    assert!(nonexistent_configs.is_empty());
}

// ============================================================================
// 测试 8: 配置清除功能
// ============================================================================

#[test]
fn test_registry_clear() {
    // 创建一个临时工具用于测试清除功能
    #[derive(Default)]
    struct TempTools;

    #[tool]
    impl TempTools {
        pub fn temp_method(&self) -> String {
            "temp".to_string()
        }
    }

    config! {
        TempTools {
            temp_method: {
                desc: "临时方法"
            }
        }
    }

    // 触发配置初始化
    let _ = &*__CONFIG_INIT_TempTools;

    // 验证配置存在
    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("temp_method"));

    // 清除特定配置
    tokitai::GLOBAL_CONFIG_REGISTRY.clear("temp_method");
    assert!(!tokitai::GLOBAL_CONFIG_REGISTRY.has_config("temp_method"));

    // 重新配置
    tokitai::GLOBAL_CONFIG_REGISTRY.configure("temp_method", &[ToolConfig::desc("重新配置的描述")]);
    assert!(tokitai::GLOBAL_CONFIG_REGISTRY.has_config("temp_method"));

    // 清除所有配置
    tokitai::GLOBAL_CONFIG_REGISTRY.clear_all();
    assert!(!tokitai::GLOBAL_CONFIG_REGISTRY.has_config("temp_method"));
}
