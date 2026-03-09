//! MCP Server 示例 - 使用 McpServerBuilder
//!
//! 展示如何使用泛型 McpServerBuilder 构建 MCP 服务器。
//!
//! # 运行方式
//!
//! ```bash
//! # 从项目根目录运行
//! cargo run --example mcp_builder_demo -p tokitai-mcp-server
//! ```
//!
//! # 测试 API
//!
//! ```bash
//! # 获取工具列表
//! curl http://127.0.0.1:8080/tools
//!
//! # 调用工具
//! curl -X POST http://127.0.0.1:8080/call \
//!   -H "Content-Type: application/json" \
//!   -d '{"name": "add", "arguments": {"a": 10, "b": 20}}'
//!
//! # 健康检查
//! curl http://127.0.0.1:8080/health
//! ```

use tokitai::tool;
use tokitai::ToolProvider;
use tokitai_mcp_server::{McpServerBuilder, MultiToolProvider};

// ==================== 工具定义 ====================

/// 数学计算器工具
#[derive(Default, Clone)]
pub struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 两个数相乘
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }

    /// 计算平方根（返回整数部分）
    pub fn sqrt(&self, n: i32) -> i32 {
        (n as f64).sqrt() as i32
    }

    /// 计算幂运算 (base^exp)
    pub fn power(&self, base: i32, exp: u32) -> i32 {
        base.pow(exp)
    }
}

/// 文本处理工具
#[derive(Default, Clone)]
pub struct TextTools;

#[tool]
impl TextTools {
    /// 将文本转换为大写
    pub fn to_uppercase(&self, text: String) -> String {
        text.to_uppercase()
    }

    /// 将文本转换为小写
    pub fn to_lowercase(&self, text: String) -> String {
        text.to_lowercase()
    }

    /// 反转文本
    pub fn reverse(&self, text: String) -> String {
        text.chars().rev().collect()
    }

    /// 计算文本长度
    pub fn length(&self, text: String) -> usize {
        text.len()
    }
}

// ==================== 主流程 ====================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Windows 中文编码检测：设置控制台代码页为 UTF-8
    #[cfg(windows)]
    {
        use std::process::Command;
        let _ = Command::new("chcp").arg("65001").output();
    }

    // 初始化日志（服务器内部也会检查是否已初始化）
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("tokitai_mcp_server=info".parse().unwrap()),
        )
        .init();

    println!("=== Tokitai MCP Server Builder 示例 ===\n");

    // 创建计算器实例
    let calculator = Calculator;

    // 展示工具定义
    println!("计算器工具列表:");
    for tool in Calculator::tool_definitions() {
        println!("  - {}: {}", tool.name, tool.description);
    }
    println!();

    // 创建文本工具实例
    let text_tools = TextTools;

    println!("文本处理工具列表:");
    for tool in TextTools::tool_definitions() {
        println!("  - {}: {}", tool.name, tool.description);
    }
    println!();

    // 演示工具调用
    println!("=== 工具调用演示 ===\n");

    // 1. 数学计算
    println!("[1] 数学计算");
    let result = calculator.call_tool("add", &serde_json::json!({"a": 100, "b": 250}))?;
    println!("    add(100, 250) = {}\n", result);

    let result = calculator.call_tool("multiply", &serde_json::json!({"a": 12, "b": 8}))?;
    println!("    multiply(12, 8) = {}\n", result);

    let result = calculator.call_tool("sqrt", &serde_json::json!({"n": 16}))?;
    println!("    sqrt(16) = {}\n", result);

    // 2. 文本处理
    println!("[2] 文本处理");
    let result =
        text_tools.call_tool("to_uppercase", &serde_json::json!({"text": "hello world"}))?;
    println!("    to_uppercase('hello world') = {}\n", result);

    let result = text_tools.call_tool("reverse", &serde_json::json!({"text": "hello"}))?;
    println!("    reverse('hello') = {}\n", result);

    // 启动 HTTP 服务器的提示
    println!("=== 启动 HTTP MCP 服务器 ===\n");
    println!("按 Enter 键启动 HTTP 服务器 (Ctrl+C 停止)...");
    let _ = std::io::stdin().read_line(&mut String::new());

    // 使用 MultiToolProvider 组合多个工具提供者
    println!("创建 MultiToolProvider 并组合多个工具...\n");

    let mut multi_provider = MultiToolProvider::new();
    multi_provider.add(Calculator);
    multi_provider.add(TextTools);

    println!("已添加的工具:");
    for tool in multi_provider.tool_definitions() {
        println!("  - {}: {}", tool.name, tool.description);
    }
    println!();

    // 使用 Builder 创建服务器（支持多工具提供者）
    println!("启动服务器...\n");

    let server = McpServerBuilder::with_tool(multi_provider)
        .with_port(8080)
        .with_cors(true)
        .with_tracing(true)
        .build();

    // 运行服务器
    server.run().await?;

    Ok(())
}
