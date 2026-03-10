//! JSON Schema 验证测试
//!
//! 测试 one_of/pattern/min/max 等属性是否正确输出到 JSON Schema

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct ValidationTools;

#[tool]
impl ValidationTools {
    /// 设置用户配置
    ///
    /// @param role 用户角色
    /// @param email 邮箱地址
    /// @param age 年龄
    #[tool(
        one_of_role = ["admin", "user", "guest"],
        pattern_email = "@",
        min_age = 0,
        max_age = 150
    )]
    pub fn set_user_config(&self, role: String, email: String, age: i32) -> String {
        format!("User: {}, Role: {}, Age: {}", email, role, age)
    }

    /// 测试字符串长度验证
    ///
    /// @param name 用户名
    #[tool(min_length_name = 3, max_length_name = 20)]
    pub fn create_user(&self, name: String) -> String {
        format!("Created user: {}", name)
    }

    /// 测试倍数验证
    ///
    /// @param count 数量
    #[tool(multiple_of_count = 5.0)]
    pub fn batch_process(&self, count: i32) -> String {
        format!("Processing {} items", count)
    }
}

#[test]
fn test_json_schema_contains_one_of() {
    let tools = ValidationTools::tool_definitions();
    let role_tool = tools.iter().find(|t| t.name == "set_user_config").unwrap();

    let schema: serde_json::Value = serde_json::from_str(&role_tool.input_schema).unwrap();
    let role_schema = &schema["properties"]["role"];

    // 验证 enum 字段存在
    assert!(role_schema.get("enum").is_some(), "role 字段应该包含 enum");
    let enum_vals = role_schema["enum"].as_array().unwrap();
    assert_eq!(enum_vals.len(), 3, "enum 应该包含 3 个值");
    assert!(
        enum_vals.contains(&serde_json::Value::String("admin".to_string())),
        "enum 应该包含 admin"
    );
    assert!(
        enum_vals.contains(&serde_json::Value::String("user".to_string())),
        "enum 应该包含 user"
    );
    assert!(
        enum_vals.contains(&serde_json::Value::String("guest".to_string())),
        "enum 应该包含 guest"
    );
}

#[test]
fn test_json_schema_contains_pattern() {
    let tools = ValidationTools::tool_definitions();
    let tool = tools.iter().find(|t| t.name == "set_user_config").unwrap();

    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let email_schema = &schema["properties"]["email"];

    // 验证 pattern 字段存在
    assert!(
        email_schema.get("pattern").is_some(),
        "email 字段应该包含 pattern"
    );
    assert_eq!(
        email_schema["pattern"].as_str().unwrap(),
        "@",
        "pattern 应该是 @"
    );
}

#[test]
fn test_json_schema_contains_min_max() {
    let tools = ValidationTools::tool_definitions();
    let tool = tools.iter().find(|t| t.name == "set_user_config").unwrap();

    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let age_schema = &schema["properties"]["age"];

    // 验证 minimum 和 maximum 字段存在
    assert!(
        age_schema.get("minimum").is_some(),
        "age 字段应该包含 minimum"
    );
    assert!(
        age_schema.get("maximum").is_some(),
        "age 字段应该包含 maximum"
    );
    assert_eq!(
        age_schema["minimum"].as_f64().unwrap(),
        0.0,
        "minimum 应该是 0"
    );
    assert_eq!(
        age_schema["maximum"].as_f64().unwrap(),
        150.0,
        "maximum 应该是 150"
    );
}

#[test]
fn test_json_schema_contains_min_max_length() {
    let tools = ValidationTools::tool_definitions();
    let tool = tools.iter().find(|t| t.name == "create_user").unwrap();

    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let name_schema = &schema["properties"]["name"];

    // 验证 minLength 和 maxLength 字段存在
    assert!(
        name_schema.get("minLength").is_some(),
        "name 字段应该包含 minLength"
    );
    assert!(
        name_schema.get("maxLength").is_some(),
        "name 字段应该包含 maxLength"
    );
    assert_eq!(
        name_schema["minLength"].as_u64().unwrap(),
        3,
        "minLength 应该是 3"
    );
    assert_eq!(
        name_schema["maxLength"].as_u64().unwrap(),
        20,
        "maxLength 应该是 20"
    );
}

#[test]
fn test_json_schema_contains_multiple_of() {
    let tools = ValidationTools::tool_definitions();
    let tool = tools.iter().find(|t| t.name == "batch_process").unwrap();

    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let count_schema = &schema["properties"]["count"];

    // 验证 multipleOf 字段存在
    assert!(
        count_schema.get("multipleOf").is_some(),
        "count 字段应该包含 multipleOf"
    );
    assert_eq!(
        count_schema["multipleOf"].as_f64().unwrap(),
        5.0,
        "multipleOf 应该是 5"
    );
}

#[test]
fn test_tool_count() {
    // 验证工具数量
    assert_eq!(
        ValidationTools::tool_definitions().len(),
        3,
        "应该有 3 个工具"
    );
}
