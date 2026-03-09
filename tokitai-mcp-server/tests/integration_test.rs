//! MCP Server 集成测试

use serde_json::json;
use tokitai::tool;
use tokitai::ToolProvider;
use tokitai_mcp_server::{McpServerBuilder, MultiToolProvider};

/// 测试用计算器工具
#[derive(Default, Clone)]
pub struct TestCalculator;

#[tool]
impl TestCalculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 两个数相乘
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}

/// 测试用文本工具
#[derive(Default, Clone)]
pub struct TestTextTools;

#[tool]
impl TestTextTools {
    /// 将文本转换为大写
    pub fn to_uppercase(&self, text: String) -> String {
        text.to_uppercase()
    }

    /// 反转文本
    pub fn reverse(&self, text: String) -> String {
        text.chars().rev().collect()
    }
}

#[test]
fn test_tool_definitions() {
    let tools = TestCalculator::tool_definitions();
    assert_eq!(tools.len(), 2);

    let add_tool = tools.iter().find(|t| t.name == "add").unwrap();
    assert_eq!(add_tool.description, "两个数相加");

    let multiply_tool = tools.iter().find(|t| t.name == "multiply").unwrap();
    assert_eq!(multiply_tool.description, "两个数相乘");
}

#[test]
fn test_call_add() {
    let calc = TestCalculator::default();
    let result = calc.call_tool("add", &json!({"a": 10, "b": 20})).unwrap();
    assert_eq!(result, json!(30));
}

#[test]
fn test_call_multiply() {
    let calc = TestCalculator::default();
    let result = calc
        .call_tool("multiply", &json!({"a": 6, "b": 7}))
        .unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn test_call_unknown_tool() {
    let calc = TestCalculator::default();
    let result = calc.call_tool("unknown", &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_mcp_tool_format() {
    let tools = TestCalculator::tool_definitions();
    let mcp_tools = tokitai::mcp::to_mcp_tools(tools);

    assert_eq!(mcp_tools.len(), 2);

    let add_tool = mcp_tools.iter().find(|t| t.name == "add").unwrap();
    assert_eq!(add_tool.description, "两个数相加");
    assert!(add_tool.input_schema.is_object());
}

#[tokio::test]
async fn test_server_builder() {
    let calc = TestCalculator::default();
    let _server = McpServerBuilder::with_tool(calc).with_port(9999).build();

    // 验证服务器创建成功
    // 注意：这里不实际运行服务器，只验证构建过程
}

#[tokio::test]
async fn test_server_tools() {
    let calc = TestCalculator::default();
    let server = McpServerBuilder::with_tool(calc).with_port(9998).build();

    // 验证工具列表
    assert_eq!(server.tools().len(), 2);
}

// ============================================================================
// HTTP Endpoint Tests (using axum test utilities)
// ============================================================================

#[tokio::test]
async fn test_health_endpoint() {
    let calc = TestCalculator::default();
    let server = McpServerBuilder::with_tool(calc)
        .with_tracing(false) // 禁用 tracing 避免重复初始化
        .build();

    // 获取服务器配置
    let config = server.config().clone();

    // 由于无法直接获取 router，我们跳过这个测试
    // 实际测试需要重构 server 以暴露 router
    // 这里仅验证服务器可以正确构建
    assert_eq!(config.port, 8080);
}

#[tokio::test]
async fn test_multi_tool_provider() {
    let mut provider = MultiToolProvider::new();
    provider.add(TestCalculator::default());
    provider.add(TestTextTools::default());

    // 验证工具定义
    let tools = provider.tool_definitions();
    assert_eq!(tools.len(), 4); // 2 from Calculator + 2 from TextTools

    // 验证计算器工具
    assert!(tools.iter().any(|t| t.name == "add"));
    assert!(tools.iter().any(|t| t.name == "multiply"));

    // 验证文本工具
    assert!(tools.iter().any(|t| t.name == "to_uppercase"));
    assert!(tools.iter().any(|t| t.name == "reverse"));
}

#[tokio::test]
async fn test_multi_tool_call() {
    use tokitai_core::ToolCaller;

    let mut provider = MultiToolProvider::new();
    provider.add(TestCalculator::default());
    provider.add(TestTextTools::default());

    // 测试调用计算器工具
    let result = provider
        .call_tool("add", &json!({"a": 10, "b": 20}))
        .unwrap();
    assert_eq!(result, json!(30));

    // 测试调用文本工具
    let result = provider
        .call_tool("to_uppercase", &json!({"text": "hello"}))
        .unwrap();
    assert_eq!(result, json!("HELLO"));

    // 测试调用不存在的工具
    let result = provider.call_tool("nonexistent", &json!({}));
    assert!(result.is_err());
}

#[tokio::test]
async fn test_server_with_multi_tool_provider() {
    let mut provider = MultiToolProvider::new();
    provider.add(TestCalculator::default());
    provider.add(TestTextTools::default());

    let server = McpServerBuilder::with_tool(provider)
        .with_port(9997)
        .build();

    // 验证所有工具都被注册
    assert_eq!(server.tools().len(), 4);
}
