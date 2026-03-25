//! 错误注入测试：验证 call_tool 对恶意/边界 JSON 输入的容错能力
//!
//! 运行测试：cargo test -p tokitai --test error_injection_test --features serde

#![cfg(feature = "serde")]

use serde_json::json;
use tokitai::tool;
use tokitai_core::ToolErrorKind;

/// 测试用工具集
#[derive(Default)]
struct ErrorTestTools;

#[tool]
impl ErrorTestTools {
    /// 简单加法
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 处理字符串
    pub fn process_text(&self, text: String) -> String {
        text.to_uppercase()
    }

    /// 处理可选参数
    pub fn with_option(&self, required: String, optional: Option<i32>) -> String {
        format!("{}: {:?}", required, optional)
    }

    /// 返回复杂类型
    pub fn get_data(&self) -> Vec<String> {
        vec!["a".to_string(), "b".to_string()]
    }
}

// ============================================================================
// 测试 1: 缺失必需参数
// ============================================================================

#[test]
fn test_missing_required_param() {
    let tools = ErrorTestTools;

    // 缺失必需参数 a
    let result = tools.call_tool("add", &json!({"b": 10}));
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
}

#[test]
fn test_missing_all_params() {
    let tools = ErrorTestTools;

    // 完全缺失参数
    let result = tools.call_tool("add", &json!({}));
    assert!(result.is_err());
}

// ============================================================================
// 测试 2: 参数类型错误
// ============================================================================

#[test]
fn test_wrong_type_string_for_integer() {
    let tools = ErrorTestTools;

    // 字符串代替整数
    let result = tools.call_tool("add", &json!({"a": "not_a_number", "b": 10}));
    assert!(result.is_err());
}

#[test]
fn test_wrong_type_integer_for_string() {
    let tools = ErrorTestTools;

    // 整数代替字符串
    let result = tools.call_tool("process_text", &json!({"text": 12345}));
    assert!(result.is_err());
}

#[test]
fn test_wrong_type_array_for_string() {
    let tools = ErrorTestTools;

    // 数组代替字符串
    let result = tools.call_tool("process_text", &json!({"text": ["not", "a", "string"]}));
    assert!(result.is_err());
}

#[test]
fn test_wrong_type_object_for_integer() {
    let tools = ErrorTestTools;

    // 对象代替整数
    let result = tools.call_tool("add", &json!({"a": {"value": 10}, "b": 20}));
    assert!(result.is_err());
}

// ============================================================================
// 测试 3: null 值处理
// ============================================================================

#[test]
fn test_null_for_required_param() {
    let tools = ErrorTestTools;

    // null 作为必需参数
    let result = tools.call_tool("add", &json!({"a": null, "b": 10}));
    assert!(result.is_err());
}

#[test]
fn test_null_for_optional_param() {
    let tools = ErrorTestTools;

    // null 作为可选参数 - 应该成功，被视为 None
    let result = tools.call_tool(
        "with_option",
        &json!({"required": "test", "optional": null}),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!("test: None"));
}

#[test]
fn test_null_for_string_param() {
    let tools = ErrorTestTools;

    // null 作为字符串参数
    let result = tools.call_tool("process_text", &json!({"text": null}));
    assert!(result.is_err());
}

// ============================================================================
// 测试 4: 恶意/超大输入
// ============================================================================

#[test]
fn test_extremely_large_number() {
    let tools = ErrorTestTools;

    // 超出 i32 范围的大数
    let result = tools.call_tool("add", &json!({"a": i64::MAX, "b": 10}));
    // 应该失败，因为无法解析为 i32
    assert!(result.is_err());
}

#[test]
fn test_extremely_long_string() {
    let tools = ErrorTestTools;

    // 超长字符串 (1MB)
    let long_string = "a".repeat(1_000_000);
    let result = tools.call_tool("process_text", &json!({"text": long_string}));
    // 应该成功（如果内存允许）
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.as_str().unwrap().starts_with("AAA"));
}

#[test]
fn test_deeply_nested_json() {
    let tools = ErrorTestTools;

    // 深层嵌套的 JSON
    let mut nested = json!(0);
    for _ in 0..100 {
        nested = json!({"nested": nested});
    }

    // 尝试传入嵌套 JSON 作为参数
    let result = tools.call_tool("add", &nested);
    assert!(result.is_err());
}

// ============================================================================
// 测试 5: 未知工具调用
// ============================================================================

#[test]
fn test_unknown_tool() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("nonexistent_tool", &json!({}));
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert_eq!(err.kind, ToolErrorKind::NotFound);
}

#[test]
fn test_empty_tool_name() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("", &json!({}));
    assert!(result.is_err());
}

// ============================================================================
// 测试 6: 特殊字符和 Unicode
// ============================================================================

#[test]
fn test_unicode_in_tool_name() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("添加", &json!({}));
    assert!(result.is_err()); // 工具名不匹配
}

#[test]
fn test_unicode_in_string_param() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("process_text", &json!({"text": "你好，世界！🦀"}));
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.as_str().unwrap().contains("🦀"));
}

#[test]
fn test_emoji_in_tool_name() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("add_🔥", &json!({}));
    assert!(result.is_err());
}

// ============================================================================
// 测试 7: JSON 结构错误
// ============================================================================

#[test]
fn test_array_instead_of_object() {
    let tools = ErrorTestTools;

    // 数组代替对象
    let result = tools.call_tool("add", &json!([{"a": 1, "b": 2}]));
    assert!(result.is_err());
}

#[test]
fn test_string_instead_of_object() {
    let tools = ErrorTestTools;

    // 字符串代替对象
    let result = tools.call_tool("add", &json!("not an object"));
    assert!(result.is_err());
}

#[test]
fn test_integer_instead_of_object() {
    let tools = ErrorTestTools;

    // 整数代替对象
    let result = tools.call_tool("add", &json!(42));
    assert!(result.is_err());
}

// ============================================================================
// 测试 8: 边界值测试
// ============================================================================

#[test]
fn test_i32_max_value() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("add", &json!({"a": i32::MAX, "b": 0}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!(i32::MAX));
}

#[test]
fn test_i32_min_value() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("add", &json!({"a": i32::MIN, "b": 0}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!(i32::MIN));
}

#[test]
#[should_panic(expected = "attempt to add with overflow")]
fn test_integer_overflow() {
    let tools = ErrorTestTools;

    // 在 debug 模式下会 panic，在 release 模式下会溢出
    // 这个测试验证 debug 模式下的溢出检测
    let result = tools.call_tool("add", &json!({"a": i32::MAX, "b": 1}));
    let _ = result;
}

#[test]
fn test_zero_values() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("add", &json!({"a": 0, "b": 0}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!(0));
}

#[test]
fn test_negative_values() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("add", &json!({"a": -100, "b": -50}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!(-150));
}

// ============================================================================
// 测试 9: 额外参数处理
// ============================================================================

#[test]
fn test_extra_unknown_param() {
    let tools = ErrorTestTools;

    // 包含未知参数 - 应该成功，忽略额外参数
    let result = tools.call_tool("add", &json!({"a": 10, "b": 20, "unknown": "ignored"}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!(30));
}

#[test]
fn test_all_extra_params() {
    let tools = ErrorTestTools;

    // 全部是未知参数
    let result = tools.call_tool("add", &json!({"x": 1, "y": 2, "z": 3}));
    assert!(result.is_err()); // 缺少必需参数
}

// ============================================================================
// 测试 10: 空值和空集合
// ============================================================================

#[test]
fn test_empty_array_param() {
    let tools = ErrorTestTools;

    // 空数组作为返回值测试
    let result = tools.call_tool("get_data", &json!({}));
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.is_array());
    assert_eq!(response.as_array().unwrap().len(), 2);
}

#[test]
fn test_empty_string_param() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("process_text", &json!({"text": ""}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!(""));
}

#[test]
fn test_whitespace_string() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("process_text", &json!({"text": "   "}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!("   "));
}

// ============================================================================
// 测试 11: 浮点数边界
// ============================================================================

#[test]
fn test_float_for_integer_param() {
    let tools = ErrorTestTools;

    // 浮点数作为整数参数
    let result = tools.call_tool("add", &json!({"a": 10.5, "b": 20}));
    assert!(result.is_err());
}

#[test]
fn test_integer_for_float_param() {
    // 注意：JSON 中整数可以隐式转换为浮点数
    // 这个测试验证类型系统的严格性
    let tools = ErrorTestTools;

    let result = tools.call_tool("add", &json!({"a": 10, "b": 20.0}));
    // 可能成功或失败，取决于实现
    let _ = result;
}

// ============================================================================
// 测试 12: 布尔值边界
// ============================================================================

#[test]
fn test_boolean_for_integer() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("add", &json!({"a": true, "b": false}));
    assert!(result.is_err());
}

#[test]
fn test_boolean_for_string() {
    let tools = ErrorTestTools;

    let result = tools.call_tool("process_text", &json!({"text": true}));
    assert!(result.is_err());
}
