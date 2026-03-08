//! 最小化 validate 测试

use tokitai::tool;

#[tool]
pub struct TestTools;

#[tool]
impl TestTools {
    /// 测试 validate
    ///
    /// @param name 名称
    /// @validate name !value.is_empty()
    pub fn test_validate(&self, name: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("Hello, {}", name))
    }
}

fn main() {
    let _tools = TestTools;
    for def in TestTools::TOOL_DEFINITIONS {
        println!("工具：{}", def.name);
    }
}
