//! 自定义工具属性测试

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct DataProcessor;

#[tool]
impl DataProcessor {
    #[tool(name = "process_data", desc = "处理数据并返回结果")]
    pub fn process(&self, input: String) -> String {
        format!("Processed: {}", input)
    }

    /// 使用默认描述（从 doc comment 提取）
    pub fn transform(&self, data: Vec<i32>) -> Vec<i32> {
        data.iter().map(|x| x * 2).collect()
    }
}

fn main() {
    let processor = DataProcessor;

    // 验证 TOOL_DEFINITIONS 生成
    let tools = DataProcessor::tool_definitions();
    assert_eq!(tools.len(), 2);

    // 验证自定义名称
    let process_tool = tools.iter().find(|t| t.name == "process_data").unwrap();
    assert_eq!(process_tool.description, "处理数据并返回结果");

    // 验证默认名称
    let transform_tool = tools.iter().find(|t| t.name == "transform").unwrap();
    assert!(transform_tool.description.contains("默认描述") || transform_tool.description.contains("transform"));

    // 调用测试
    let result = processor.call_tool("process_data", &serde_json::json!({"input": "test"})).unwrap();
    assert_eq!(result, "Processed: test");

    let result = processor.call_tool("transform", &serde_json::json!({"data": [1, 2, 3]})).unwrap();
    assert_eq!(result, serde_json::json!([2, 4, 6]));
}
