//! Custom tool attribute test. The description contains non-ASCII
//! characters (Chinese) which triggers the T-022 NON_ASCII_DESC
//! lint, so we opt out with `allow_insecure_desc`.

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct DataProcessor;

#[tool(allow_insecure_desc)]
impl DataProcessor {
    #[tool(name = "process_data", desc = "Processes input String parameter and returns the result; requires the input parameter to be non-empty.", allow_short_desc)]
    pub fn process(&self, input: String) -> String {
        format!("Processed: {}", input)
    }

    /// Uses default description (extracted from doc comment)
    pub fn transform(&self, data: Vec<i32>) -> Vec<i32> {
        data.iter().map(|x| x * 2).collect()
    }
}

fn main() {
    let processor = DataProcessor;

    // Verify TOOL_DEFINITIONS generation
    let tools = DataProcessor::tool_definitions();
    assert_eq!(tools.len(), 2);

    // Verify custom name
    let process_tool = tools.iter().find(|t| t.name == "process_data").unwrap();
    assert_eq!(process_tool.description, "Processes input String parameter and returns the result; requires the input parameter to be non-empty.");

    // Verify default name
    let transform_tool = tools.iter().find(|t| t.name == "transform").unwrap();
    assert!(transform_tool.description.contains("Uses default") || transform_tool.description.contains("transform"));

    // Call test
    let result = processor.call_tool("process_data", &serde_json::json!({"input": "test"})).unwrap();
    assert_eq!(result, "Processed: test");

    let result = processor.call_tool("transform", &serde_json::json!({"data": [1, 2, 3]})).unwrap();
    assert_eq!(result, serde_json::json!([2, 4, 6]));
}
