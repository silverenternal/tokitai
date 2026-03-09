//! MCP HTTP 服务器示例
//!
//! 展示如何运行一个完整的 MCP HTTP 服务器。
//!
//! # 运行方式
//!
//! ```bash
//! cargo run --example mcp_http_server
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

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokitai::tool;
use tokitai::ToolProvider;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ==================== 工具定义 ====================

#[derive(Default, Clone)]
struct Calculator;

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

    /// 计算平方根
    pub fn sqrt(&self, n: i32) -> i32 {
        (n as f64).sqrt() as i32
    }
}

#[derive(Default, Clone)]
struct HashCalculator;

#[tool]
impl HashCalculator {
    /// 计算字符串的 SHA256 哈希值
    pub fn sha256(&self, input: String) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }
}

#[derive(Default, Clone)]
struct WeatherService;

#[tool]
impl WeatherService {
    /// 获取指定城市的天气信息
    pub fn get_weather(&self, city: String) -> String {
        match city.to_lowercase().as_str() {
            "北京" | "beijing" => "北京：晴朗，温度 25°C，湿度 40%".to_string(),
            "上海" | "shanghai" => "上海：多云，温度 22°C，湿度 60%".to_string(),
            "广州" | "guangzhou" => "广州：小雨，温度 28°C，湿度 80%".to_string(),
            "深圳" | "shenzhen" => "深圳：晴朗，温度 30°C，湿度 70%".to_string(),
            "纽约" | "new york" => "纽约：晴朗，温度 20°C，湿度 45%".to_string(),
            "伦敦" | "london" => "伦敦：阴天，温度 15°C，湿度 70%".to_string(),
            "东京" | "tokyo" => "东京：多云，温度 22°C，湿度 60%".to_string(),
            _ => format!("{}：数据不可用", city),
        }
    }
}

#[derive(Default, Clone)]
struct TimeService;

#[tool]
impl TimeService {
    /// 获取当前日期时间
    pub fn get_current_time(&self) -> String {
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// 计算两个日期的天数差
    pub fn days_between(&self, date1: String, date2: String) -> Result<i32, String> {
        let d1 = chrono::NaiveDate::parse_from_str(&date1, "%Y-%m-%d")
            .map_err(|e| format!("日期格式错误：{}", e))?;
        let d2 = chrono::NaiveDate::parse_from_str(&date2, "%Y-%m-%d")
            .map_err(|e| format!("日期格式错误：{}", e))?;
        Ok((d2 - d1).num_days() as i32)
    }
}

// ==================== 应用状态 ====================

/// 应用状态
struct AppState {
    tools: Vec<tokitai::mcp::McpTool>,
}

/// 工具调用请求
#[derive(Debug, Deserialize)]
struct ToolCallRequest {
    name: String,
    #[serde(default)]
    arguments: Value,
}

/// 工具调用响应
#[derive(Debug, Serialize)]
struct ToolCallResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ToolCallResponse {
    fn success(result: Value) -> Self {
        Self {
            success: true,
            result: Some(result),
            error: None,
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(message.into()),
        }
    }
}

// ==================== HTTP 处理器 ====================

/// 列出工具处理器
async fn list_tools_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<tokitai::mcp::McpTool>> {
    info!("Listing {} tools", state.tools.len());
    Json(state.tools.clone())
}

/// 调用工具处理器
async fn call_tool_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ToolCallRequest>,
) -> Result<Json<ToolCallResponse>, StatusCode> {
    info!(
        "Tool call request: name={}, arguments={:?}",
        request.name, request.arguments
    );

    // 查找工具
    let tool = state
        .tools
        .iter()
        .find(|t| t.name == request.name)
        .ok_or_else(|| {
            warn!("Tool not found: {}", request.name);
            StatusCode::NOT_FOUND
        })?;

    info!("Found tool: {} - {}", tool.name, tool.description);

    // 实际调用工具
    // 注意：这里需要具体的工具实例来调用
    // 在实际应用中，你需要维护一个工具实例映射
    let result = call_concrete_tool(&request.name, &request.arguments);

    match result {
        Ok(value) => Ok(Json(ToolCallResponse::success(value))),
        Err(e) => {
            error!("Tool execution error: {}", e);
            Ok(Json(ToolCallResponse::error(e)))
        }
    }
}

/// 健康检查处理器
async fn health_handler() -> &'static str {
    "OK"
}

// ==================== 工具调用路由 ====================

/// 调用具体工具的辅助函数
fn call_concrete_tool(name: &str, args: &Value) -> Result<Value, String> {
    let calculator = Calculator;
    let hash_calculator = HashCalculator;
    let weather = WeatherService;
    let time_service = TimeService;

    match name {
        "add" | "multiply" | "sqrt" => calculator
            .call_tool(name, args)
            .map_err(|e| format!("{:?}", e)),
        "sha256" => hash_calculator
            .call_tool(name, args)
            .map_err(|e| format!("{:?}", e)),
        "get_weather" => weather
            .call_tool(name, args)
            .map_err(|e| format!("{:?}", e)),
        "get_current_time" | "days_between" => time_service
            .call_tool(name, args)
            .map_err(|e| format!("{:?}", e)),
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

// ==================== 主流程 ====================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tokitai=info,axum=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("=== Tokitai MCP HTTP 服务器 ===\n");

    // 收集所有工具定义
    let mut all_tools = Vec::new();
    all_tools.extend(tokitai::mcp::to_mcp_tools(Calculator::tool_definitions()).into_iter());
    all_tools.extend(tokitai::mcp::to_mcp_tools(HashCalculator::tool_definitions()).into_iter());
    all_tools.extend(tokitai::mcp::to_mcp_tools(WeatherService::tool_definitions()).into_iter());
    all_tools.extend(tokitai::mcp::to_mcp_tools(TimeService::tool_definitions()).into_iter());

    println!("已加载 {} 个工具:", all_tools.len());
    for tool in &all_tools {
        println!("  - {}: {}", tool.name, tool.description);
    }
    println!();

    // 创建应用状态
    let state = Arc::new(AppState { tools: all_tools });

    // 构建路由
    let app = Router::new()
        .route("/tools", get(list_tools_handler))
        .route("/call", post(call_tool_handler))
        .route("/health", get(health_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // 启动服务器
    let addr = "127.0.0.1:8080";
    println!("MCP HTTP 服务器启动中...");
    println!("监听地址：http://{}", addr);
    println!();
    println!("API 端点:");
    println!("  GET  /tools  - 获取工具列表");
    println!("  POST /call   - 调用工具");
    println!("  GET  /health - 健康检查");
    println!();
    println!("测试命令:");
    println!("  curl http://127.0.0.1:8080/tools");
    println!("  curl -X POST http://127.0.0.1:8080/call -H \"Content-Type: application/json\" -d '{{\"name\":\"add\",\"arguments\":{{\"a\":10,\"b\":20}}}}'");
    println!();
    println!("按 Ctrl+C 停止服务器\n");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
