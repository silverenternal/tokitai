//! Schema 缓存测试：验证 TYPE_SCHEMA_CACHE 的线程安全和正确性
//!
//! 运行测试：cargo test -p tokitai-macros --test schema_cache_test

use serde::Serialize;
use std::sync::{Arc, Barrier};
use std::thread;
use tokitai::tool;
use tokitai::ToolProvider;
use tokitai_macros::tool_type;

// ============================================================================
// 测试用自定义类型 - 使用 tool_type 注册 schema
// ============================================================================

/// 位置信息 - 使用 tool_type 注册 schema
#[tool_type(
    name = "Location",
    properties = "latitude: number, longitude: number",
    required = "latitude, longitude"
)]
#[derive(Default, Clone)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

/// 用户信息 - 使用 tool_type 注册 schema
#[tool_type(
    name = "User",
    properties = "id: integer, name: string, email: string, age: integer",
    required = "id, name, email"
)]
#[derive(Default, Clone, Serialize)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
}

/// 复杂嵌套类型
#[tool_type(
    name = "Article",
    properties = "title: string, content: string, author: User, tags: array",
    required = "title, content, author"
)]
#[derive(Default, Clone)]
pub struct Article {
    pub title: String,
    pub content: String,
    pub author: User,
    pub tags: Vec<String>,
}

// ============================================================================
// 测试 1: 基本工具定义测试（使用自定义类型作为参数）
// ============================================================================

#[derive(Default)]
struct CustomTypeTools;

#[tool]
impl CustomTypeTools {
    /// 处理位置信息
    pub fn process_location(&self, latitude: f64, longitude: f64) -> String {
        format!("Processing location: {}, {}", latitude, longitude)
    }

    /// 创建用户
    pub fn create_user(&self, name: String, email: String) -> String {
        format!("Created user: {} <{}>", name, email)
    }
}

#[test]
fn test_tool_definitions_generated() {
    let tool_defs = CustomTypeTools::tool_definitions();
    assert_eq!(tool_defs.len(), 2);

    // 验证工具名称
    let names: Vec<&str> = tool_defs.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"process_location"));
    assert!(names.contains(&"create_user"));
}

#[test]
fn test_tool_schema_for_basic_types() {
    let tool_defs = CustomTypeTools::tool_definitions();
    let location_tool = tool_defs
        .iter()
        .find(|t| t.name == "process_location")
        .unwrap();

    // 验证 schema 是有效的 JSON
    let schema_value: serde_json::Value =
        serde_json::from_str(&location_tool.input_schema).unwrap();
    assert_eq!(schema_value["type"], "object");
    assert!(schema_value["properties"]["latitude"].is_object());
    assert!(schema_value["properties"]["longitude"].is_object());

    // 验证类型正确
    assert_eq!(schema_value["properties"]["latitude"]["type"], "number");
    assert_eq!(schema_value["properties"]["longitude"]["type"], "number");
}

// ============================================================================
// 测试 2: Schema 缓存并发访问测试
// ============================================================================

#[test]
fn test_concurrent_tool_definitions_access() {
    let num_threads = 10;
    let accesses_per_thread = 100;
    let barrier = Arc::new(Barrier::new(num_threads));

    // 创建多个线程同时访问 tool_definitions
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();

                for _ in 0..accesses_per_thread {
                    let _defs = CustomTypeTools::tool_definitions();
                }
            })
        })
        .collect();

    // 等待所有线程完成
    for handle in handles {
        handle.join().expect("线程不应该 panic");
    }

    // 验证工具定义仍然可用
    let tool_defs = CustomTypeTools::tool_definitions();
    assert_eq!(tool_defs.len(), 2);
}

#[test]
fn test_concurrent_different_tools() {
    #[derive(Default)]
    struct ToolSetA;

    #[tool]
    impl ToolSetA {
        pub fn method_a1(&self) -> String {
            "a1".to_string()
        }
        pub fn method_a2(&self) -> String {
            "a2".to_string()
        }
    }

    #[derive(Default)]
    struct ToolSetB;

    #[tool]
    impl ToolSetB {
        pub fn method_b1(&self) -> String {
            "b1".to_string()
        }
        pub fn method_b2(&self) -> String {
            "b2".to_string()
        }
    }

    let num_threads = 20;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();

                match i % 2 {
                    0 => {
                        for _ in 0..50 {
                            let _ = ToolSetA::tool_definitions();
                        }
                    }
                    1 => {
                        for _ in 0..50 {
                            let _ = ToolSetB::tool_definitions();
                        }
                    }
                    _ => unreachable!(),
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("线程不应该 panic");
    }

    // 验证两个工具集都正常
    assert_eq!(ToolSetA::tool_definitions().len(), 2);
    assert_eq!(ToolSetB::tool_definitions().len(), 2);
}

// ============================================================================
// 测试 3: Schema 内容验证
// ============================================================================

#[test]
fn test_schema_is_valid_json() {
    let tool_defs = CustomTypeTools::tool_definitions();

    for tool_def in tool_defs {
        let result: Result<serde_json::Value, _> = serde_json::from_str(&tool_def.input_schema);
        assert!(
            result.is_ok(),
            "Schema 应该是有效的 JSON: {}",
            tool_def.input_schema
        );
    }
}

#[test]
fn test_schema_property_types() {
    let tool_defs = CustomTypeTools::tool_definitions();
    let location_tool = tool_defs
        .iter()
        .find(|t| t.name == "process_location")
        .unwrap();

    let schema_value: serde_json::Value =
        serde_json::from_str(&location_tool.input_schema).unwrap();

    // 验证 latitude 是 number 类型
    assert_eq!(schema_value["properties"]["latitude"]["type"], "number");

    // 验证 longitude 是 number 类型
    assert_eq!(schema_value["properties"]["longitude"]["type"], "number");
}

#[test]
fn test_schema_required_fields() {
    let tool_defs = CustomTypeTools::tool_definitions();
    let create_user_tool = tool_defs.iter().find(|t| t.name == "create_user").unwrap();

    let schema_value: serde_json::Value =
        serde_json::from_str(&create_user_tool.input_schema).unwrap();

    let required = schema_value["required"].as_array().unwrap();

    // 验证必需字段
    assert!(required.contains(&serde_json::json!("name")));
    assert!(required.contains(&serde_json::json!("email")));
}

// ============================================================================
// 测试 4: Schema 缓存一致性测试
// ============================================================================

#[test]
fn test_schema_cache_consistency() {
    // 多次获取同一工具的 schema，验证返回相同结果
    let schema1 = CustomTypeTools::tool_definitions();
    let schema2 = CustomTypeTools::tool_definitions();
    let schema3 = CustomTypeTools::tool_definitions();

    // 比较 JSON 字符串
    assert_eq!(schema1[0].input_schema, schema2[0].input_schema);
    assert_eq!(schema2[0].input_schema, schema3[0].input_schema);
}

#[test]
fn test_schema_serialization() {
    let tool_defs = CustomTypeTools::tool_definitions();
    let schema_str = &tool_defs[0].input_schema;

    // 验证可以反序列化
    let value: serde_json::Value = serde_json::from_str(schema_str).unwrap();

    // 验证可以重新序列化
    let reserialized = serde_json::to_string(&value).unwrap();
    assert!(reserialized.contains("\"type\":\"object\""));
}

#[test]
fn test_schema_pretty_print() {
    let tool_defs = CustomTypeTools::tool_definitions();
    let schema_str = &tool_defs[0].input_schema;
    let value: serde_json::Value = serde_json::from_str(schema_str).unwrap();

    let pretty = serde_json::to_string_pretty(&value).unwrap();

    // 验证格式化后的 JSON 包含换行
    assert!(pretty.contains('\n'));
    assert!(pretty.contains("  ")); // 缩进
}

// ============================================================================
// 测试 5: Schema 缓存边界测试
// ============================================================================

#[test]
fn test_empty_param_tool() {
    #[derive(Default)]
    struct EmptyTool;

    #[tool]
    impl EmptyTool {
        pub fn no_params(&self) -> String {
            "no params".to_string()
        }
    }

    let tool_defs = EmptyTool::tool_definitions();
    assert_eq!(tool_defs.len(), 1);

    let schema_value: serde_json::Value = serde_json::from_str(&tool_defs[0].input_schema).unwrap();
    assert_eq!(schema_value["type"], "object");
    assert!(schema_value["properties"].as_object().unwrap().is_empty());
    assert!(schema_value["required"].as_array().unwrap().is_empty());
}

#[test]
fn test_single_param_tool() {
    #[derive(Default)]
    struct SingleParamTool;

    #[tool]
    impl SingleParamTool {
        pub fn single_param(&self, value: String) -> String {
            value
        }
    }

    let tool_defs = SingleParamTool::tool_definitions();
    assert_eq!(tool_defs.len(), 1);

    let schema_value: serde_json::Value = serde_json::from_str(&tool_defs[0].input_schema).unwrap();
    assert_eq!(schema_value["type"], "object");
    assert_eq!(schema_value["properties"]["value"]["type"], "string");

    let required = schema_value["required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert!(required.contains(&serde_json::json!("value")));
}

// ============================================================================
// 测试 6: Schema 缓存无死锁测试
// ============================================================================

#[test]
fn test_schema_cache_no_deadlock() {
    // 这个测试验证缓存不会导致死锁
    let schema1 = CustomTypeTools::tool_definitions();
    let _schema2 = CustomTypeTools::tool_definitions();

    // 再次获取，验证不会阻塞
    let _schema_again = CustomTypeTools::tool_definitions();

    // 验证所有 schema 都有效
    for schema in schema1.iter() {
        assert!(serde_json::from_str::<serde_json::Value>(&schema.input_schema).is_ok());
    }
}

// ============================================================================
// 测试 7: 复杂类型 Schema 测试
// ============================================================================

#[test]
fn test_option_param_schema() {
    #[derive(Default)]
    struct OptionTool;

    #[tool]
    impl OptionTool {
        pub fn with_option(&self, required: String, optional: Option<i32>) -> String {
            format!("{}: {:?}", required, optional)
        }
    }

    let tool_defs = OptionTool::tool_definitions();
    assert_eq!(tool_defs.len(), 1);

    let schema_value: serde_json::Value = serde_json::from_str(&tool_defs[0].input_schema).unwrap();

    // required 应该在 required 数组中
    let required = schema_value["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::json!("required")));
    assert!(!required.contains(&serde_json::json!("optional")));
}

#[test]
fn test_vec_param_schema() {
    #[derive(Default)]
    struct VecTool;

    #[tool]
    impl VecTool {
        pub fn process_list(&self, items: Vec<String>) -> String {
            format!("Processed {} items", items.len())
        }
    }

    let tool_defs = VecTool::tool_definitions();
    let schema_value: serde_json::Value = serde_json::from_str(&tool_defs[0].input_schema).unwrap();

    // 验证 items 是 array 类型
    assert_eq!(schema_value["properties"]["items"]["type"], "array");
}

// ============================================================================
// 测试 8: 工具调用与 Schema 验证集成测试
// ============================================================================

#[test]
fn test_tool_call_with_valid_args() {
    let tools = CustomTypeTools;

    let result = tools.call_tool(
        "process_location",
        &serde_json::json!({
            "latitude": 40.7128,
            "longitude": -74.0060
        }),
    );

    assert!(result.is_ok());
}

#[test]
fn test_tool_call_with_missing_param() {
    let tools = CustomTypeTools;

    // 缺失必需参数
    let result = tools.call_tool(
        "process_location",
        &serde_json::json!({
            "latitude": 40.7128
        }),
    );

    assert!(result.is_err());
}

#[test]
fn test_tool_call_with_invalid_type() {
    let tools = CustomTypeTools;

    // 类型错误
    let result = tools.call_tool(
        "process_location",
        &serde_json::json!({
            "latitude": "not_a_number",
            "longitude": -74.0060
        }),
    );

    assert!(result.is_err());
}
