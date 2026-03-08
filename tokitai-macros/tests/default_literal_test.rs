//! 测试 default 直接字面量支持

use tokitai::tool;
use serde_json::json;

#[tool]
pub struct DefaultLiteralTools;

#[tool]
impl DefaultLiteralTools {
    /// 测试默认值 null
    ///
    /// @param name 名称
    /// @param nickname 昵称（可选，默认为 null）
    #[tool(default_nickname = "null")]
    pub fn test_null_default(&self, name: String, nickname: Option<String>) -> Result<String, tokitai::ToolError> {
        Ok(format!("name={}, nickname={:?}", name, nickname))
    }

    /// 测试默认值整数（字符串格式）
    ///
    /// @param name 名称
    /// @param count 数量（默认为 10）
    #[tool(default_count = "10")]
    pub fn test_int_default_str(&self, name: String, count: i32) -> Result<String, tokitai::ToolError> {
        Ok(format!("name={}, count={}", name, count))
    }

    /// 测试默认值整数（直接字面量）
    ///
    /// @param name 名称
    /// @param count 数量（默认为 10）
    #[tool(default_count = 10)]
    pub fn test_int_default(&self, name: String, count: i32) -> Result<String, tokitai::ToolError> {
        Ok(format!("name={}, count={}", name, count))
    }

    /// 测试默认值浮点数（字符串格式）
    ///
    /// @param name 名称
    /// @param price 价格（默认为 99.99）
    #[tool(default_price = "99.99")]
    pub fn test_float_default_str(&self, name: String, price: f64) -> Result<String, tokitai::ToolError> {
        Ok(format!("name={}, price={}", name, price))
    }

    /// 测试默认值浮点数（直接字面量）
    ///
    /// @param name 名称
    /// @param price 价格（默认为 99.99）
    #[tool(default_price = 99.99)]
    pub fn test_float_default(&self, name: String, price: f64) -> Result<String, tokitai::ToolError> {
        Ok(format!("name={}, price={}", name, price))
    }

    /// 测试默认值布尔值（字符串格式）
    ///
    /// @param name 名称
    /// @param active 是否激活（默认为 true）
    #[tool(default_active = "true")]
    pub fn test_bool_default_str(&self, name: String, active: bool) -> Result<String, tokitai::ToolError> {
        Ok(format!("name={}, active={}", name, active))
    }

    /// 测试默认值布尔值（直接字面量）
    ///
    /// @param name 名称
    /// @param active 是否激活（默认为 true）
    #[tool(default_active = true)]
    pub fn test_bool_default(&self, name: String, active: bool) -> Result<String, tokitai::ToolError> {
        Ok(format!("name={}, active={}", name, active))
    }

    /// 测试默认值数组（字符串格式）
    ///
    /// @param name 名称
    /// @param tags 标签（默认为 ["default"]）
    #[tool(default_tags = "[\"default\"]")]
    pub fn test_array_default_str(&self, name: String, tags: Vec<String>) -> Result<String, tokitai::ToolError> {
        Ok(format!("name={}, tags={:?}", name, tags))
    }

    /// 测试默认值数组（直接字面量）
    ///
    /// @param name 名称
    /// @param tags 标签（默认为 ["default"]）
    #[tool(default_tags = ["default"])]
    pub fn test_array_default(&self, name: String, tags: Vec<String>) -> Result<String, tokitai::ToolError> {
        Ok(format!("name={}, tags={:?}", name, tags))
    }
}

#[test]
fn test_null_default_in_schema() {
    let tools = DefaultLiteralTools::TOOL_DEFINITIONS;
    let tool = tools.iter().find(|t| t.name == "test_null_default").unwrap();
    
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    
    // 验证 nickname 字段有 default: null
    assert_eq!(schema["properties"]["nickname"]["default"], serde_json::Value::Null);
}

#[test]
fn test_int_default_str_in_schema() {
    let tools = DefaultLiteralTools::TOOL_DEFINITIONS;
    let tool = tools.iter().find(|t| t.name == "test_int_default_str").unwrap();
    
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    
    // 验证 count 字段有 default: 10
    assert_eq!(schema["properties"]["count"]["default"], json!(10));
}

#[test]
fn test_int_default_in_schema() {
    let tools = DefaultLiteralTools::TOOL_DEFINITIONS;
    let tool = tools.iter().find(|t| t.name == "test_int_default").unwrap();
    
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    
    // 验证 count 字段有 default: 10
    assert_eq!(schema["properties"]["count"]["default"], json!(10));
}

#[test]
fn test_float_default_str_in_schema() {
    let tools = DefaultLiteralTools::TOOL_DEFINITIONS;
    let tool = tools.iter().find(|t| t.name == "test_float_default_str").unwrap();
    
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    
    // 验证 price 字段有 default: 99.99
    assert_eq!(schema["properties"]["price"]["default"], json!(99.99));
}

#[test]
fn test_float_default_in_schema() {
    let tools = DefaultLiteralTools::TOOL_DEFINITIONS;
    let tool = tools.iter().find(|t| t.name == "test_float_default").unwrap();
    
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    
    // 验证 price 字段有 default: 99.99
    assert_eq!(schema["properties"]["price"]["default"], json!(99.99));
}

#[test]
fn test_bool_default_str_in_schema() {
    let tools = DefaultLiteralTools::TOOL_DEFINITIONS;
    let tool = tools.iter().find(|t| t.name == "test_bool_default_str").unwrap();
    
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    
    // 验证 active 字段有 default: true
    assert_eq!(schema["properties"]["active"]["default"], json!(true));
}

#[test]
fn test_bool_default_in_schema() {
    let tools = DefaultLiteralTools::TOOL_DEFINITIONS;
    let tool = tools.iter().find(|t| t.name == "test_bool_default").unwrap();
    
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    
    // 验证 active 字段有 default: true
    assert_eq!(schema["properties"]["active"]["default"], json!(true));
}

#[test]
fn test_array_default_str_in_schema() {
    let tools = DefaultLiteralTools::TOOL_DEFINITIONS;
    let tool = tools.iter().find(|t| t.name == "test_array_default_str").unwrap();
    
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    
    // 验证 tags 字段有 default: ["default"]
    assert_eq!(schema["properties"]["tags"]["default"], json!(["default"]));
}

#[test]
fn test_array_default_in_schema() {
    let tools = DefaultLiteralTools::TOOL_DEFINITIONS;
    let tool = tools.iter().find(|t| t.name == "test_array_default").unwrap();
    
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    
    // 验证 tags 字段有 default: ["default"]
    assert_eq!(schema["properties"]["tags"]["default"], json!(["default"]));
}
