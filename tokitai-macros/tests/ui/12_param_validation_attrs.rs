//! 测试 12: 参数属性 - 完整的参数验证和转换

use tokitai::tool;
use tokitai::ToolProvider;
use tokitai::param_tool;

#[derive(Default)]
pub struct ParamValidationTools;

#[tool]
impl ParamValidationTools {
    /// 带有完整参数验证的方法
    pub fn create_user(
        &self,
        #[param_tool(desc = "用户名，3-20 个字符", min_length = 3, max_length = 20)]
        username: String,
        #[param_tool(desc = "邮箱地址", pattern = "@")]
        email: String,
        #[param_tool(desc = "年龄", min = 0, max = 150)]
        age: i32,
        #[param_tool(desc = "可选的备注", default = "无备注")]
        note: Option<String>
    ) -> String {
        format!("User: {}, Email: {}, Age: {}, Note: {:?}", username, email, age, note)
    }
    
    /// 带有转换的方法
    pub fn process_email(
        &self,
        #[param_tool(transform = "value.to_lowercase()")]
        email: String
    ) -> String {
        email
    }
}

fn main() {
    let tools = ParamValidationTools;
    
    // 验证工具定义生成
    let defs = ParamValidationTools::tool_definitions();
    assert_eq!(defs.len(), 2);
    
    // 验证 create_user 的 schema
    let create_tool = defs.iter().find(|t| t.name == "create_user").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&create_tool.input_schema).unwrap();
    
    // 验证 username 参数
    assert_eq!(schema["properties"]["username"]["type"], "string");
    assert_eq!(schema["properties"]["username"]["minLength"], 3);
    assert_eq!(schema["properties"]["username"]["maxLength"], 20);
    
    // 验证 email 参数
    assert_eq!(schema["properties"]["email"]["type"], "string");
    assert!(schema["properties"]["email"]["pattern"].as_str().unwrap().contains("@"));
    
    // 验证 age 参数
    assert_eq!(schema["properties"]["age"]["type"], "integer");
    assert_eq!(schema["properties"]["age"]["minimum"], 0);
    assert_eq!(schema["properties"]["age"]["maximum"], 150);
    
    // 调用测试
    let result = tools.call_tool(
        "create_user",
        &serde_json::json!({
            "username": "john_doe",
            "email": "john@example.com",
            "age": 30,
            "note": "VIP 用户"
        })
    ).unwrap();
    assert!(result.as_str().unwrap().contains("john_doe"));
    
    // 测试转换
    let result = tools.call_tool(
        "process_email",
        &serde_json::json!({"email": "TEST@EXAMPLE.COM"})
    ).unwrap();
    assert_eq!(result, "test@example.com");
}
