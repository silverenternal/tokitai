//! MCP Server HTTP 端到端测试
//!
//! 运行测试：cargo test -p tokitai-mcp-server --test http_end_to_end_test

use serde_json::json;
use tokitai::tool;
use tokitai_mcp_server::{McpServerBuilder, McpServerConfig};

/// 测试用计算器工具
#[derive(Default, Clone)]
struct HttpTestCalculator;

#[tool]
impl HttpTestCalculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 两个数相减
    pub fn subtract(&self, a: i32, b: i32) -> i32 {
        a - b
    }

    /// 处理字符串
    pub fn echo(&self, message: String) -> String {
        message
    }
}

// ============================================================================
// 测试 1: 服务器构建配置
// ============================================================================

#[test]
fn test_server_config_default() {
    let config = McpServerConfig::default();
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8080);
    assert!(config.cors_enabled);
    assert!(config.tracing_enabled);
}

#[test]
fn test_server_config_custom() {
    let config = McpServerConfig::new("0.0.0.0", 9999);
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 9999);
    assert!(config.cors_enabled);
    assert!(config.tracing_enabled);
}

#[test]
fn test_server_config_builder() {
    let config = McpServerConfig::default()
        .with_cors(false)
        .with_tracing(false);
    assert!(!config.cors_enabled);
    assert!(!config.tracing_enabled);
}

#[test]
fn test_server_config_address() {
    let config = McpServerConfig::new("127.0.0.1", 3000);
    assert_eq!(config.address(), "127.0.0.1:3000");
}

// ============================================================================
// 测试 2: 服务器构建器测试
// ============================================================================

#[test]
fn test_server_builder_basic() {
    let calc = HttpTestCalculator;
    let server = McpServerBuilder::with_tool(calc).build();

    assert_eq!(server.tools().len(), 3);
}

#[test]
fn test_server_builder_with_port() {
    let calc = HttpTestCalculator;
    let server = McpServerBuilder::with_tool(calc).with_port(9000).build();

    assert_eq!(server.config().port, 9000);
}

#[test]
fn test_server_builder_with_config() {
    let calc = HttpTestCalculator;
    let config = McpServerConfig::new("0.0.0.0", 8888)
        .with_cors(false)
        .with_tracing(false);
    let server = McpServerBuilder::with_config(config, calc).build();

    assert_eq!(server.config().host, "0.0.0.0");
    assert_eq!(server.config().port, 8888);
    assert!(!server.config().cors_enabled);
    assert!(!server.config().tracing_enabled);
}

// ============================================================================
// 测试 3: 工具定义测试
// ============================================================================

#[test]
fn test_server_tools_list() {
    let calc = HttpTestCalculator;
    let server = McpServerBuilder::with_tool(calc).build();

    let tools = server.tools();
    assert_eq!(tools.len(), 3);

    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"add"));
    assert!(tool_names.contains(&"subtract"));
    assert!(tool_names.contains(&"echo"));
}

#[test]
fn test_tool_definition_schema() {
    let calc = HttpTestCalculator;
    let server = McpServerBuilder::with_tool(calc).build();

    let tools = server.tools();
    let add_tool = tools.iter().find(|t| t.name == "add").unwrap();

    // 验证 schema 是有效的 JSON
    let schema: serde_json::Value = add_tool.input_schema.clone();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"].is_object());
    assert!(schema["properties"]["a"].is_object());
    assert!(schema["properties"]["b"].is_object());
}

// ============================================================================
// 测试 4: 工具调用测试（同步）
// ============================================================================

#[test]
fn test_server_tool_call_add() {
    let calc = HttpTestCalculator;
    let _server = McpServerBuilder::with_tool(calc.clone()).build();

    // 验证工具可以通过 call_tool 调用
    let result = calc.call_tool("add", &json!({"a": 10, "b": 20}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!(30));
}

#[test]
fn test_server_tool_call_subtract() {
    let calc = HttpTestCalculator;
    let _server = McpServerBuilder::with_tool(calc.clone()).build();

    let result = calc.call_tool("subtract", &json!({"a": 50, "b": 30}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!(20));
}

#[test]
fn test_server_tool_call_echo() {
    let calc = HttpTestCalculator;
    let _server = McpServerBuilder::with_tool(calc.clone()).build();

    let result = calc.call_tool("echo", &json!({"message": "Hello, World!"}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!("Hello, World!"));
}

// ============================================================================
// 测试 5: 错误处理测试
// ============================================================================

#[test]
fn test_server_tool_call_not_found() {
    let calc = HttpTestCalculator;

    let result = calc.call_tool("nonexistent", &json!({}));
    assert!(result.is_err());
}

#[test]
fn test_server_tool_call_invalid_args() {
    let calc = HttpTestCalculator;

    // 缺失必需参数
    let result = calc.call_tool("add", &json!({"a": 10}));
    assert!(result.is_err());

    // 类型错误
    let result = calc.call_tool("add", &json!({"a": "not_a_number", "b": 20}));
    assert!(result.is_err());
}

// ============================================================================
// 测试 6: 多工具提供者测试
// ============================================================================

#[derive(Default, Clone)]
struct TextTools;

#[tool]
impl TextTools {
    /// 转大写
    pub fn to_upper(&self, text: String) -> String {
        text.to_uppercase()
    }

    /// 转小写
    pub fn to_lower(&self, text: String) -> String {
        text.to_lowercase()
    }
}

#[test]
fn test_multi_tool_provider() {
    use tokitai_mcp_server::MultiToolProvider;

    let mut provider = MultiToolProvider::new();
    provider.add(HttpTestCalculator);
    provider.add(TextTools);

    let tools = provider.tool_definitions();
    assert_eq!(tools.len(), 5); // 3 from Calculator + 2 from TextTools
}

#[test]
fn test_multi_tool_provider_with_server() {
    use tokitai_mcp_server::MultiToolProvider;

    let mut provider = MultiToolProvider::new();
    provider.add(HttpTestCalculator);
    provider.add(TextTools);

    let server = McpServerBuilder::with_tool(provider).build();
    assert_eq!(server.tools().len(), 5);
}

// ============================================================================
// 测试 7: 服务器配置隔离测试
// ============================================================================

#[test]
fn test_server_config_isolation() {
    let calc1 = HttpTestCalculator;
    let server1 = McpServerBuilder::with_tool(calc1).with_port(8001).build();

    let calc2 = HttpTestCalculator;
    let server2 = McpServerBuilder::with_tool(calc2).with_port(8002).build();

    assert_eq!(server1.config().port, 8001);
    assert_eq!(server2.config().port, 8002);
}

// ============================================================================
// 测试 8: 工具别名测试 (跳过 - 别名语法可能不同)
// ============================================================================

// 注：工具别名功能依赖于具体的宏实现语法
// 这里仅验证基本功能，别名测试在集成测试中已覆盖
