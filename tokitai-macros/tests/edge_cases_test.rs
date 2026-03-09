//! 边界情况测试
//! 测试 multiple_of 边界、空字符串验证、组合验证等

use serde_json::json;
use tokitai::tool;

// ========================================
// multiple_of 边界测试
// ========================================

#[tool]
pub struct MultipleOfTools;

#[tool]
impl MultipleOfTools {
    /// 测试 multiple_of 0.01 倍数
    ///
    /// @param value 数值（必须是 0.01 的倍数）
    #[tool(multiple_of_value = 0.01)]
    pub fn test_price(&self, value: f64) -> Result<String, tokitai::ToolError> {
        Ok(format!("value={}", value))
    }

    /// 测试 multiple_of 5 倍数
    ///
    /// @param value 数值（必须是 5 的倍数）
    #[tool(multiple_of_value = 5.0)]
    pub fn test_multiple_of_5(&self, value: i32) -> Result<String, tokitai::ToolError> {
        Ok(format!("value={}", value))
    }
}

#[test]
fn test_multiple_of_edge_cases() {
    let tools = MultipleOfTools;

    // 应该成功 - 0.01 的倍数
    assert!(tools
        .call_tool("test_price", &json!({"value": 0.99}))
        .is_ok());
    assert!(tools
        .call_tool("test_price", &json!({"value": 1.50}))
        .is_ok());
    assert!(tools
        .call_tool("test_price", &json!({"value": 99.99}))
        .is_ok());
    assert!(tools
        .call_tool("test_price", &json!({"value": 100.0}))
        .is_ok());
    assert!(tools
        .call_tool("test_price", &json!({"value": 0.01}))
        .is_ok());

    // 应该失败 - 不是 0.01 的倍数
    assert!(tools
        .call_tool("test_price", &json!({"value": 0.991}))
        .is_err());
    assert!(tools
        .call_tool("test_price", &json!({"value": 1.001}))
        .is_err());

    // 应该成功 - 5 的倍数
    assert!(tools
        .call_tool("test_multiple_of_5", &json!({"value": 10}))
        .is_ok());
    assert!(tools
        .call_tool("test_multiple_of_5", &json!({"value": 100}))
        .is_ok());
    assert!(tools
        .call_tool("test_multiple_of_5", &json!({"value": 0}))
        .is_ok());

    // 应该失败 - 不是 5 的倍数
    assert!(tools
        .call_tool("test_multiple_of_5", &json!({"value": 7}))
        .is_err());
    assert!(tools
        .call_tool("test_multiple_of_5", &json!({"value": 13}))
        .is_err());
}

// ========================================
// 空字符串验证测试
// ========================================

#[tool]
pub struct ValidateMsgTools;

#[tool]
impl ValidateMsgTools {
    /// 创建用户
    ///
    /// @param name 用户名（不能为空）
    /// @param email 邮箱地址
    /// @validate name !name.is_empty()
    /// @validate_msg name "用户名不能为空，请至少输入 3 个字符"
    #[tool(min_length_name = 3)]
    pub fn create_user(&self, name: String, email: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("创建用户：name={}, email={}", name, email))
    }
}

#[test]
fn test_empty_string_validation() {
    let tools = ValidateMsgTools;

    // 空字符串验证 - 应该失败
    let result = tools.call_tool(
        "create_user",
        &json!({
            "name": "",
            "email": "test@example.com"
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("用户名不能为空") || err_msg.contains("长度"));

    // 太短 - 应该失败
    let result = tools.call_tool(
        "create_user",
        &json!({
            "name": "ab",
            "email": "test@example.com"
        }),
    );
    assert!(result.is_err());

    // 正常 - 应该成功
    let result = tools.call_tool(
        "create_user",
        &json!({
            "name": "zhangsan",
            "email": "test@example.com"
        }),
    );
    assert!(result.is_ok());
}

// ========================================
// 组合验证测试
// ========================================

#[tool]
pub struct CombinedTools;

#[tool]
impl CombinedTools {
    /// 复杂验证示例
    ///
    /// @param param1 参数 1（3-10 字符）
    /// @param param2 参数 2（0-100）
    /// @param param3 参数 3（枚举值）
    #[tool(
        min_length_param1 = 3,
        max_length_param1 = 10,
        min_param2 = 0,
        max_param2 = 100,
        one_of_param3 = ["valid", "ok", "good"]
    )]
    pub fn complex_method(
        &self,
        param1: String,
        param2: i32,
        param3: String,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!(
            "param1={}, param2={}, param3={}",
            param1, param2, param3
        ))
    }
}

#[test]
fn test_combined_validation() {
    let tools = CombinedTools;

    // 同时触发多个验证错误 - 应该失败
    let result = tools.call_tool(
        "complex_method",
        &json!({
            "param1": "",       // 触发 min_length
            "param2": 200,      // 触发 max
            "param3": "invalid" // 触发 one_of
        }),
    );

    assert!(result.is_err());

    // 部分验证失败
    let result = tools.call_tool(
        "complex_method",
        &json!({
            "param1": "valid_name",
            "param2": 50,
            "param3": "invalid" // 触发 one_of
        }),
    );
    assert!(result.is_err());

    // 全部验证通过
    let result = tools.call_tool(
        "complex_method",
        &json!({
            "param1": "valid",
            "param2": 50,
            "param3": "valid"
        }),
    );
    assert!(result.is_ok());
}

// ========================================
// Option 类型边界测试
// ========================================

#[tool]
pub struct OptionTools;

#[tool]
impl OptionTools {
    /// 测试 Option 类型
    ///
    /// @param value 可选数值（如果提供，必须是 10 的倍数）
    #[tool(multiple_of_value = 10.0)]
    pub fn test_optional(&self, value: Option<i32>) -> Result<String, tokitai::ToolError> {
        Ok(format!("value={:?}", value))
    }
}

#[test]
fn test_option_multiple_of() {
    let tools = OptionTools;

    // None 应该成功
    assert!(tools.call_tool("test_optional", &json!({})).is_ok());
    assert!(tools
        .call_tool("test_optional", &json!({"value": null}))
        .is_ok());

    // 有效的倍数应该成功
    assert!(tools
        .call_tool("test_optional", &json!({"value": 20}))
        .is_ok());
    assert!(tools
        .call_tool("test_optional", &json!({"value": 100}))
        .is_ok());

    // 无效的倍数应该失败
    assert!(tools
        .call_tool("test_optional", &json!({"value": 15}))
        .is_err());
    assert!(tools
        .call_tool("test_optional", &json!({"value": 7}))
        .is_err());
}

// ========================================
// 数值边界测试
// ========================================

#[tool]
pub struct NumericBoundaryTools;

#[tool]
impl NumericBoundaryTools {
    /// 测试数值边界
    ///
    /// @param min_val 最小值测试（最小 0）
    /// @param max_val 最大值测试（最大 100）
    #[tool(min_min_val = 0, max_max_val = 100)]
    pub fn test_boundaries(
        &self,
        min_val: i32,
        max_val: i32,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!("min_val={}, max_val={}", min_val, max_val))
    }
}

#[test]
fn test_numeric_boundaries() {
    let tools = NumericBoundaryTools;

    // 边界值应该成功
    assert!(tools
        .call_tool(
            "test_boundaries",
            &json!({
                "min_val": 0,
                "max_val": 100
            })
        )
        .is_ok());

    // 边界内应该成功
    assert!(tools
        .call_tool(
            "test_boundaries",
            &json!({
                "min_val": 1,
                "max_val": 99
            })
        )
        .is_ok());

    // 超出最小值应该失败
    let result = tools.call_tool(
        "test_boundaries",
        &json!({
            "min_val": -1,
            "max_val": 50
        }),
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("小于最小值"));

    // 超出最大值应该失败
    let result = tools.call_tool(
        "test_boundaries",
        &json!({
            "min_val": 0,
            "max_val": 101
        }),
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("大于最大值"));
}
