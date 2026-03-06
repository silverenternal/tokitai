//! 测试 #[tool(skip)] 属性

use tokitai::tool;

pub struct Processor;

#[tool]
impl Processor {
    /// 这个方法会被注册为工具
    pub fn public_method(&self) -> String {
        "public".to_string()
    }

    /// 这个方法会被跳过，不会注册
    #[tool(skip)]
    pub fn skipped_method(&self) -> String {
        "skipped".to_string()
    }

    /// 另一个会被跳过的内部方法
    #[tool(skip)]
    fn internal_helper(&self) -> i32 {
        42
    }
}

fn main() {
    let processor = Processor;

    // 验证 TOOL_DEFINITIONS 生成 - 只有 public_method 被注册
    let tools = Processor::TOOL_DEFINITIONS;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "public_method");

    // 验证 skipped_method 不在工具列表中
    assert!(!tools.iter().any(|t| t.name == "skipped_method"));
    assert!(!tools.iter().any(|t| t.name == "internal_helper"));

    // 验证 call_tool 只能调用 public_method
    let result = processor.call_tool("public_method", &serde_json::json!({})).unwrap();
    assert_eq!(result, "public");

    // 调用 skipped_method 应该失败
    let result = processor.call_tool("skipped_method", &serde_json::json!({}));
    assert!(result.is_err());
}
