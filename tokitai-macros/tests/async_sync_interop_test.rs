//! 异步/同步互操作测试：验证同步方法的互操作性
//!
//! 注：当前宏实现不支持 async 方法，此测试主要验证同步方法的互操作性
//!
//! 运行测试：cargo test -p tokitai-macros --test async_sync_interop_test

use tokitai::tool;
use tokitai::ToolProvider;

// ============================================================================
// 测试用工具集 - 同步方法
// ============================================================================

#[derive(Default, Clone)]
pub struct SyncTools;

#[tool]
impl SyncTools {
    /// 加法
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 字符串处理
    pub fn process_text(&self, text: String) -> String {
        text.to_uppercase()
    }

    /// 计算
    pub fn compute(&self, value: f64) -> f64 {
        value * 2.0
    }
}

// ============================================================================
// 测试 1: 基本同步方法调用
// ============================================================================

#[test]
fn test_sync_method_call() {
    let tools = SyncTools;

    let result = tools.call_tool("add", &serde_json::json!({"a": 10, "b": 20}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!(30));
}

#[test]
fn test_text_processing() {
    let tools = SyncTools;

    let result = tools.call_tool("process_text", &serde_json::json!({"text": "hello world"}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!("HELLO WORLD"));
}

#[test]
fn test_compute() {
    let tools = SyncTools;

    let result = tools.call_tool("compute", &serde_json::json!({"value": 42.5}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!(85.0));
}

// ============================================================================
// 测试 2: 并发调用
// ============================================================================

#[test]
fn test_concurrent_calls() {
    use std::sync::Arc;
    use std::thread;

    let tools = Arc::new(SyncTools);
    let num_threads = 10;

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let tools = Arc::clone(&tools);
            thread::spawn(move || {
                let result = tools.call_tool("add", &serde_json::json!({"a": i, "b": 1}));
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), serde_json::json!(i + 1));
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("线程不应该 panic");
    }
}

// ============================================================================
// 测试 3: 工具定义验证
// ============================================================================

#[test]
fn test_tool_definitions_include_all() {
    let defs = SyncTools::tool_definitions();

    // 验证所有工具都被注册
    assert_eq!(defs.len(), 3);

    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"process_text"));
    assert!(names.contains(&"compute"));
}

#[test]
fn test_tool_schema_generation() {
    let defs = SyncTools::tool_definitions();

    // 验证每个工具的 schema 都是有效的 JSON
    for def in defs {
        let schema_result: Result<serde_json::Value, _> = serde_json::from_str(&def.input_schema);
        assert!(schema_result.is_ok(), "Schema 应该是有效的 JSON");

        let schema = schema_result.unwrap();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
    }
}

// ============================================================================
// 测试 4: 错误处理
// ============================================================================

#[test]
fn test_error_handling() {
    let tools = SyncTools;

    // 缺失参数
    let result = tools.call_tool("add", &serde_json::json!({"a": 10}));
    assert!(result.is_err());

    // 未知工具
    let result = tools.call_tool("nonexistent", &serde_json::json!({}));
    assert!(result.is_err());

    // 类型错误
    let result = tools.call_tool("add", &serde_json::json!({"a": "not_a_number", "b": 20}));
    assert!(result.is_err());
}

// ============================================================================
// 测试 5: 工具提供者 trait
// ============================================================================

#[test]
fn test_tool_provider_trait() {
    // 验证 ToolProvider trait 可以正常使用
    let defs = SyncTools::tool_definitions();
    assert_eq!(defs.len(), 3);

    let tool = SyncTools::find_tool("add");
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().name, "add");

    let tool = SyncTools::find_tool("process_text");
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().name, "process_text");
}

#[test]
fn test_tool_count() {
    assert_eq!(SyncTools::tool_count(), 3);
}

// ============================================================================
// 测试 6: 多工具协作
// ============================================================================

#[derive(Default, Clone)]
struct DataProcessor;

#[tool]
impl DataProcessor {
    /// 数据转换
    pub fn transform(&self, data: String) -> String {
        data.to_uppercase()
    }

    /// 数据验证
    pub fn validate(&self, data: String) -> bool {
        !data.is_empty()
    }

    /// 数据处理
    pub fn process(&self, input: String, times: i32) -> String {
        (0..times).fold(input, |acc, _| acc.to_uppercase())
    }
}

#[test]
fn test_multi_tool_collaboration() {
    let processor = DataProcessor;

    // 数据转换
    let transform_result = processor.call_tool("transform", &serde_json::json!({"data": "hello"}));
    assert!(transform_result.is_ok());
    assert_eq!(transform_result.unwrap(), serde_json::json!("HELLO"));

    // 数据验证
    let validate_result = processor.call_tool("validate", &serde_json::json!({"data": "test"}));
    assert!(validate_result.is_ok());
    assert_eq!(validate_result.unwrap(), serde_json::json!(true));

    // 数据处理
    let process_result =
        processor.call_tool("process", &serde_json::json!({"input": "abc", "times": 2}));
    assert!(process_result.is_ok());
    assert_eq!(process_result.unwrap(), serde_json::json!("ABC"));
}

// ============================================================================
// 测试 7: 边界值测试
// ============================================================================

#[test]
fn test_boundary_values() {
    let tools = SyncTools;

    // 零值
    let result = tools.call_tool("add", &serde_json::json!({"a": 0, "b": 0}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!(0));

    // 负值
    let result = tools.call_tool("add", &serde_json::json!({"a": -10, "b": -5}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!(-15));

    // 最大值
    let result = tools.call_tool("add", &serde_json::json!({"a": i32::MAX, "b": 0}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!(i32::MAX));

    // 最小值
    let result = tools.call_tool("add", &serde_json::json!({"a": i32::MIN, "b": 0}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!(i32::MIN));
}

// ============================================================================
// 测试 8: 空字符串和空白字符
// ============================================================================

#[test]
fn test_empty_and_whitespace() {
    let tools = SyncTools;

    // 空字符串
    let result = tools.call_tool("process_text", &serde_json::json!({"text": ""}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!(""));

    // 空白字符
    let result = tools.call_tool("process_text", &serde_json::json!({"text": "   "}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!("   "));

    // 包含空白
    let result = tools.call_tool("process_text", &serde_json::json!({"text": "hello world"}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!("HELLO WORLD"));
}

// ============================================================================
// 测试 9: Unicode 支持
// ============================================================================

#[test]
fn test_unicode_support() {
    let tools = SyncTools;

    // 中文字符
    let result = tools.call_tool("process_text", &serde_json::json!({"text": "你好世界"}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!("你好世界"));

    // Emoji
    let result = tools.call_tool("process_text", &serde_json::json!({"text": "Hello 🦀"}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), serde_json::json!("HELLO 🦀"));
}

// ============================================================================
// 测试 10: 大量数据测试
// ============================================================================

#[test]
fn test_large_data() {
    let tools = SyncTools;

    // 长字符串
    let long_string = "a".repeat(10000);
    let result = tools.call_tool(
        "process_text",
        &serde_json::json!({"text": long_string.clone()}),
    );
    assert!(result.is_ok());
    let output = result.unwrap().as_str().unwrap().to_string();
    assert_eq!(output, long_string.to_uppercase());
}
