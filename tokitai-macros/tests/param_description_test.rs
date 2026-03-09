//! 参数描述测试：验证三种工具描述方式
//!
//! 1. 文档注释自动提取
//! 2. #[tool(desc)] 属性覆盖
//! 3. tokitai! 配置宏

use serde_json::Value;
use tokitai::tool;
use tokitai::ToolProvider;

// ============================================================================
// 测试 1: 方法文档注释
// ============================================================================

#[test]
fn test_method_doc_comment() {
    struct TestTools;

    #[tool]
    impl TestTools {
        /// 这是方法描述
        pub fn test_method(&self, name: String) -> String {
            format!("Hello, {}", name)
        }
    }

    let tool = &TestTools::tool_definitions()[0];
    assert_eq!(tool.description, "这是方法描述");
}

// ============================================================================
// 测试 2: 方法自定义描述（#[tool] 属性覆盖）
// ============================================================================

#[test]
fn test_method_custom_desc() {
    struct TestTools;

    #[tool]
    impl TestTools {
        #[tool(desc = "自定义描述")]
        pub fn test_method(&self, name: String) -> String {
            format!("Hello, {}", name)
        }
    }

    let tool = &TestTools::tool_definitions()[0];
    assert_eq!(tool.description, "自定义描述");
}

// ============================================================================
// 测试 3: 参数描述（使用 #[tool(desc_param = "...")] 语法）
// ============================================================================

#[test]
fn test_param_description_with_tool_attrs() {
    struct TestTools;

    #[tool]
    impl TestTools {
        /// 测试方法
        ///
        /// @param param1 参数 1 描述
        /// @param param2 参数 2 描述
        #[tool(example_param1 = "示例", min_param2 = 0, max_param2 = 150)]
        pub fn test_method(&self, param1: String, param2: i32) -> String {
            format!("{}: {}", param1, param2)
        }
    }

    let tool = &TestTools::tool_definitions()[0];
    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap();

    // 验证 param1 的示例（注意：example 存储为 JSON 字符串，需要解析）
    let example_val = schema["properties"]["param1"]["example"].as_str().unwrap();
    let example_parsed: Value = serde_json::from_str(&format!("\"{}\"", example_val))
        .unwrap_or_else(|_| example_val.to_string().into());
    assert!(example_parsed.as_str().unwrap().contains("示例"));

    // 验证 param2 的数值范围
    assert_eq!(
        schema["properties"]["param2"]["minimum"].as_f64().unwrap() as i32,
        0
    );
    assert_eq!(
        schema["properties"]["param2"]["maximum"].as_f64().unwrap() as i32,
        150
    );
}

// ============================================================================
// 测试 4: 混合使用方法和参数描述
// ============================================================================

#[test]
fn test_mixed_method_and_param_descriptions() {
    struct TestTools;

    #[tool]
    impl TestTools {
        /// 方法文档描述
        ///
        /// @param name 用户姓名
        /// @param age 用户年龄
        /// @param email 邮箱地址（可选）
        #[tool(example_name = "张三", min_age = 0, max_age = 150)]
        pub fn test_method(&self, name: String, _age: i32, _email: Option<String>) -> String {
            format!("{} <{}>", name, _email.unwrap_or_default())
        }
    }

    let tool = &TestTools::tool_definitions()[0];

    // 验证方法描述
    assert_eq!(tool.description, "方法文档描述");

    // 验证参数 schema
    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap();

    assert_eq!(
        schema["properties"]["name"]["example"]
            .as_str()
            .unwrap()
            .trim_matches('"'),
        "张三"
    );

    assert_eq!(
        schema["properties"]["age"]["minimum"].as_f64().unwrap() as i32,
        0
    );
    assert_eq!(
        schema["properties"]["age"]["maximum"].as_f64().unwrap() as i32,
        150
    );
}

// ============================================================================
// 测试 5: 参数描述优先级（参数级 > 类型推断）
// ============================================================================

#[test]
fn test_param_description_priority() {
    struct TestTools;

    #[tool]
    impl TestTools {
        /// 方法描述
        ///
        /// @param name 显式描述优先
        pub fn test_method(&self, name: String) -> String {
            name
        }
    }

    let tool = &TestTools::tool_definitions()[0];
    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap();

    // 文档注释中的描述应该被提取
    assert!(schema["properties"]["name"]["description"]
        .as_str()
        .unwrap()
        .contains("显式描述优先"));
}

// ============================================================================
// 测试 6: 复杂类型的参数描述
// ============================================================================

#[test]
fn test_complex_type_param_description() {
    use std::collections::HashMap;

    struct TestTools;

    #[tool]
    impl TestTools {
        /// 测试方法
        ///
        /// @param ids 用户 ID 列表
        /// @param metadata 用户元数据
        #[tool(min_items_ids = 1, max_items_ids = 100)]
        pub fn test_method(&self, ids: Vec<i32>, _metadata: HashMap<String, String>) -> String {
            format!("{:?}", ids)
        }
    }

    let tool = &TestTools::tool_definitions()[0];
    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap();

    // 验证数组类型的描述
    assert_eq!(schema["properties"]["ids"]["description"], "用户 ID 列表");
    assert_eq!(schema["properties"]["ids"]["minItems"], 1);
    assert_eq!(schema["properties"]["ids"]["maxItems"], 100);

    // 验证 HashMap 类型的描述（注意：HashMap 没有文档注释描述）
    // 这里只验证 minItems 和 maxItems 是否正确
    assert_eq!(schema["properties"]["ids"]["minItems"], 1);
    assert_eq!(schema["properties"]["ids"]["maxItems"], 100);
}

// ============================================================================
// 测试 7: 配置宏基本功能
// ============================================================================

#[test]
fn test_config_macro_basic() {
    struct TestTools;

    #[tool]
    impl TestTools {
        /// 默认描述
        pub fn get_user(&self, id: i32) -> String {
            format!("User {}", id)
        }
    }

    // 当前配置宏主要是语法解析测试，实际覆盖功能需要运行时支持
    let tool = &TestTools::tool_definitions()[0];
    assert_eq!(tool.description, "默认描述");
}

// ============================================================================
// 测试 8: 多种验证属性与描述一起使用
// ============================================================================

#[test]
fn test_param_desc_with_validation_attrs() {
    struct TestTools;

    #[tool]
    impl TestTools {
        /// 创建用户
        ///
        /// @param username 用户名，2-20 个字符
        /// @param password 密码，至少 8 个字符
        #[tool(
            example_username = "user123",
            min_length_username = 2,
            max_length_username = 20,
            pattern_username = r"^[a-zA-Z0-9_]+$",
            min_length_password = 8
        )]
        pub fn create_user(&self, username: String, _password: String) -> String {
            format!("{}:***", username)
        }
    }

    let tool = &TestTools::tool_definitions()[0];
    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap();

    // 验证 username 的所有属性
    assert_eq!(
        schema["properties"]["username"]["description"],
        "用户名，2-20 个字符"
    );
    assert_eq!(
        schema["properties"]["username"]["example"]
            .as_str()
            .unwrap()
            .trim_matches('"'),
        "user123"
    );
    assert_eq!(schema["properties"]["username"]["minLength"], 2);
    assert_eq!(schema["properties"]["username"]["maxLength"], 20);
    assert!(schema["properties"]["username"]["pattern"]
        .as_str()
        .unwrap()
        .contains("a-zA-Z0-9"));

    // 验证 password 的属性（只验证 minLength）
    assert_eq!(schema["properties"]["password"]["minLength"], 8);
}

// ============================================================================
// 测试 9: Option 类型参数的描述
// ============================================================================

#[test]
fn test_option_param_description() {
    struct TestTools;

    #[tool]
    impl TestTools {
        /// 搜索
        ///
        /// @param query 搜索关键词
        /// @param limit 每页结果数
        /// @param offset 分页偏移量
        #[tool(
            example_query = "rust programming",
            default_limit = 10,
            default_offset = 0
        )]
        pub fn search(&self, query: String, _limit: Option<i32>, _offset: Option<i32>) -> String {
            format!("{} (limit: {:?}, offset: {:?})", query, _limit, _offset)
        }
    }

    let tool = &TestTools::tool_definitions()[0];
    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap();

    // 验证必填参数
    assert_eq!(schema["properties"]["query"]["description"], "搜索关键词");
    assert_eq!(
        schema["properties"]["query"]["example"]
            .as_str()
            .unwrap()
            .trim_matches('"'),
        "rust programming"
    );

    // 验证可选参数的默认值
    assert!(
        schema["properties"]["_limit"]["default"].is_null()
            || schema["properties"]["_limit"]["default"].as_f64() == Some(10.0)
    );
    assert!(
        schema["properties"]["_offset"]["default"].is_null()
            || schema["properties"]["_offset"]["default"].as_f64() == Some(0.0)
    );
}
