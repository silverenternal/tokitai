//! 测试 10: 配置宏 - 配置不存在的方法（应该编译通过但配置不生效）

use tokitai::tool;
use tokitai::config;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct ConfigErrorTools;

#[tool]
impl ConfigErrorTools {
    /// 存在的方法
    pub fn existing_method(&self) -> String {
        "exists".to_string()
    }
}

// 配置一个不存在的方法 - 这应该编译通过但配置不会生效
config! {
    ConfigErrorTools {
        nonexistent_method: {
            desc: "这个配置不会生效"
        }
    }
}

fn main() {
    // 验证存在的方法仍然可用
    let defs = ConfigErrorTools::tool_definitions();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "existing_method");
}
