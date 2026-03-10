//! 测试 06: 宏展开错误 - 无效的工具名称

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct InvalidNameTools;

#[tool]
impl InvalidNameTools {
    /// 这个方法名称包含特殊字符
    #[tool(name = "invalid-name-with-dash")]
    pub fn method1(&self) -> String {
        "test".to_string()
    }
}

fn main() {
    let tools = InvalidNameTools;
    let _defs = InvalidNameTools::tool_definitions();
}
