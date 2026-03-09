//! 测试 validate_msg 自定义错误消息功能

use tokitai::tool;

#[tool]
pub struct ValidateMsgTools;

#[tool]
impl ValidateMsgTools {
    /// 创建用户（带自定义错误消息）
    ///
    /// @param name 用户名（不能为空）
    /// @validate name !value.is_empty()
    /// @validate_msg name "用户名不能为空，请至少输入 3 个字符"
    /// @param email 用户邮箱
    /// @validate email value.contains("@")
    /// @validate_msg email "邮箱格式不正确，必须包含 @ 符号"
    pub fn create_user(&self, name: String, email: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("创建用户：{} (邮箱：{})", name, email))
    }

    /// 设置年龄（带范围验证和自定义错误消息）
    ///
    /// @param age 用户年龄
    /// @validate age value > 0 && value < 150
    /// @validate_msg age "年龄必须在 0 到 150 之间"
    pub fn set_age(&self, age: i32) -> Result<String, tokitai::ToolError> {
        Ok(format!("设置年龄：{}", age))
    }
}

#[test]
fn test_validate_msg_with_empty_name() {
    let tools = ValidateMsgTools;

    // 测试空用户名，应该显示自定义错误消息
    let result = tools.call_tool(
        "create_user",
        &serde_json::json!({
            "name": "",
            "email": "test@example.com"
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    println!("Error message: {}", err_msg); // 调试输出
    assert!(err_msg.contains("用户名") || err_msg.contains("不能为空"));
}

#[test]
fn test_validate_msg_with_invalid_email() {
    let tools = ValidateMsgTools;

    // 测试无效邮箱，应该显示自定义错误消息
    let result = tools.call_tool(
        "create_user",
        &serde_json::json!({
            "name": "张三",
            "email": "invalid-email"
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    println!("Error message: {}", err_msg); // 调试输出
    assert!(err_msg.contains("邮箱") || err_msg.contains("@"));
}

#[test]
fn test_validate_msg_with_invalid_age() {
    let tools = ValidateMsgTools;

    // 测试无效年龄，应该显示自定义错误消息
    let result = tools.call_tool(
        "set_age",
        &serde_json::json!({
            "age": 200
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    println!("Error message: {}", err_msg); // 调试输出
    assert!(err_msg.contains("年龄") || err_msg.contains("0") || err_msg.contains("150"));
}

#[test]
fn test_validate_msg_success() {
    let tools = ValidateMsgTools;

    // 测试有效输入
    let result = tools
        .call_tool(
            "create_user",
            &serde_json::json!({
                "name": "张三",
                "email": "zhangsan@example.com"
            }),
        )
        .unwrap();

    assert!(result.to_string().contains("创建用户：张三"));
}
