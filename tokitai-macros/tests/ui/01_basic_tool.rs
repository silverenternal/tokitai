//! 基本工具测试

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 两个数相乘
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}

fn main() {
    let calc = Calculator;

    // 验证 TOOL_DEFINITIONS 生成
    let tools = Calculator::tool_definitions();
    assert_eq!(tools.len(), 2);

    // 验证 call_tool 生成
    let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20})).unwrap();
    assert_eq!(result, 30);
}
