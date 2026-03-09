//! 测试 input_schema_pretty 和 input_schema_value 辅助方法

use tokitai::tool;
use tokitai::ToolProvider;

#[tool]
pub struct HelperTools;

#[tool]
impl HelperTools {
    /// 测试方法
    ///
    /// @param name 用户名
    #[tool]
    pub fn test_method(&self, name: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("Hello, {}", name))
    }

    /// 多参数方法
    ///
    /// @param age 年龄
    /// @param email 邮箱
    #[tool]
    pub fn multi_param_method(
        &self,
        age: i32,
        email: String,
        active: Option<bool>,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!("age={}, email={}, active={:?}", age, email, active))
    }
}

#[test]
fn test_input_schema_pretty() {
    let tools = HelperTools::tool_definitions();
    let tool = tools.iter().find(|t| t.name == "test_method").unwrap();

    let pretty = tool.input_schema_pretty().unwrap();

    // 验证格式化输出包含换行
    assert!(pretty.contains('\n'));

    // 验证包含必要字段
    assert!(pretty.contains("\"type\""));
    assert!(pretty.contains("\"properties\""));
    assert!(pretty.contains("\"name\""));

    // 验证包含描述
    assert!(pretty.contains("用户名"));
}

#[test]
fn test_input_schema_pretty_multi_params() {
    let tools = HelperTools::tool_definitions();
    let tool = tools
        .iter()
        .find(|t| t.name == "multi_param_method")
        .unwrap();

    let pretty = tool.input_schema_pretty().unwrap();

    // 验证包含所有参数
    assert!(pretty.contains("\"age\""));
    assert!(pretty.contains("\"email\""));
    assert!(pretty.contains("\"active\""));

    // 验证参数类型
    assert!(pretty.contains("\"integer\"")); // age
    assert!(pretty.contains("\"string\"")); // email
    assert!(pretty.contains("\"boolean\"")); // active
}

#[test]
fn test_input_schema_value() {
    let tools = HelperTools::tool_definitions();
    let tool = tools.iter().find(|t| t.name == "test_method").unwrap();

    let schema = tool.input_schema_value().unwrap();

    // 验证可以访问 JSON 字段
    assert_eq!(schema["type"].as_str().unwrap(), "object");
    assert!(schema["properties"]["name"].is_object());

    // 验证描述
    let name_desc = schema["properties"]["name"]["description"]
        .as_str()
        .unwrap();
    assert_eq!(name_desc, "用户名");
}

#[test]
fn test_input_schema_value_multi_params() {
    let tools = HelperTools::tool_definitions();
    let tool = tools
        .iter()
        .find(|t| t.name == "multi_param_method")
        .unwrap();

    let schema = tool.input_schema_value().unwrap();

    // 验证根结构
    assert_eq!(schema["type"].as_str().unwrap(), "object");

    // 验证 age 参数
    assert_eq!(
        schema["properties"]["age"]["type"].as_str().unwrap(),
        "integer"
    );
    assert_eq!(
        schema["properties"]["age"]["description"].as_str().unwrap(),
        "年龄"
    );

    // 验证 email 参数
    assert_eq!(
        schema["properties"]["email"]["type"].as_str().unwrap(),
        "string"
    );

    // 验证 active 参数（可选）
    // Option 类型可能没有 type 字段，或者使用 null 类型
    let active_schema = &schema["properties"]["active"];
    assert!(active_schema.is_object());

    // 验证必需字段（active 是可选的，所以不应该在 required 中）
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str().unwrap() == "age"));
    assert!(required.iter().any(|v| v.as_str().unwrap() == "email"));
    assert!(!required.iter().any(|v| v.as_str().unwrap() == "active"));
}

#[test]
fn test_helper_methods_chaining() {
    // 测试可以链式调用辅助方法
    let tools = HelperTools::tool_definitions();

    for def in tools {
        // 可以同时调用 pretty 和 value 方法
        let pretty = def.input_schema_pretty().unwrap();
        let value = def.input_schema_value().unwrap();

        // 验证 pretty 是 value 的格式化版本
        assert!(pretty.contains(&value["type"].as_str().unwrap().to_string()));
    }
}
