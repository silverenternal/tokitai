//! MCP (Model Context Protocol) 支持
//!
//! 提供与 MCP 兼容的工具定义格式和服务器运行时。
//!
//! # 快速开始
//!
//! ```rust,ignore
//! use tokitai::{tool, mcp::McpServer};
//!
//! #[tool]
//! struct Calculator;
//!
//! #[tool]
//! impl Calculator {
//!     pub fn add(&self, a: i32, b: i32) -> i32 {
//!         a + b
//!     }
//! }
//!
//! // 创建 MCP 服务器
//! let server = Calculator::new_mcp_server();
//! ```

use serde::{Deserialize, Serialize};
use tokitai_core::ToolDefinition;

#[cfg(feature = "mcp")]
use async_trait::async_trait;

/// MCP 工具定义格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// 将 tokitai 工具定义转换为 MCP 格式
pub fn to_mcp_tools(tools: &[ToolDefinition]) -> Vec<McpTool> {
    tools
        .iter()
        .filter_map(|t| match serde_json::from_str(&t.input_schema) {
            Ok(schema) => Some(McpTool {
                name: t.name.to_string(),
                description: t.description.to_string(),
                input_schema: schema,
            }),
            Err(e) => {
                log::warn!("工具 '{}' 的 schema 解析失败：{}", t.name, e);
                None
            }
        })
        .collect()
}

/// MCP 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// MCP 工具调用响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl McpToolResponse {
    pub fn success(result: serde_json::Value) -> Self {
        Self {
            success: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(message.into()),
        }
    }
}

// ============================================================================
// MCP Server Trait - 核心抽象
// ============================================================================

/// MCP 服务器 trait
///
/// 提供 MCP 协议所需的工具列表和调用接口。
/// 此 trait 会自动为所有 `#[tool]` 类型实现，无需手动实现。
///
/// # 示例
///
/// ```rust,ignore
/// use tokitai::{tool, mcp::McpServer};
///
/// #[tool]
/// struct MyTools;
///
/// #[tool]
/// impl MyTools {
///     pub fn greet(&self, name: String) -> String {
///         format!("Hello, {}!", name)
///     }
/// }
///
/// // 自动实现了 McpServer
/// let server = MyTools::new_mcp_server();
/// let tools = server.list_tools().await;
/// let result = server.call_tool("greet", &json!({"name": "Alice"})).await;
/// ```
#[cfg(feature = "mcp")]
#[async_trait]
pub trait McpServer: Sized + Send + Sync {
    /// 获取所有可用的工具定义
    async fn list_tools(&self) -> Vec<McpTool>;

    /// 调用指定工具
    async fn call_tool(&self, name: &str, arguments: &serde_json::Value) -> McpToolResponse;

    /// 获取工具数量
    async fn tool_count(&self) -> usize {
        self.list_tools().await.len()
    }
}

// ============================================================================
// 为所有 #[tool] 类型自动实现 McpServer
// ============================================================================

/// MCP 服务器包装器
///
/// 将任何实现了 `ToolProvider` 和 `ToolCaller` 的类型包装为 MCP 服务器。
/// 这是 `#[tool]` 宏自动实现 `McpServer` 的基础。
#[cfg(feature = "mcp")]
pub struct McpServerWrapper<T> {
    inner: T,
}

#[cfg(feature = "mcp")]
impl<T> McpServerWrapper<T>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Clone + Send + Sync + 'static,
{
    /// 创建新的 MCP 服务器包装器
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// 获取内部工具实例
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// 转换为 MCP 工具定义
    pub fn to_mcp_tools(&self) -> Vec<McpTool> {
        to_mcp_tools(T::tool_definitions())
    }
}

#[cfg(feature = "mcp")]
#[async_trait]
impl<T> McpServer for McpServerWrapper<T>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Clone + Send + Sync + 'static,
{
    async fn list_tools(&self) -> Vec<McpTool> {
        self.to_mcp_tools()
    }

    async fn call_tool(&self, name: &str, arguments: &serde_json::Value) -> McpToolResponse {
        // 使用 ToolCaller trait 的 call_tool 方法
        match self.inner.call_tool(name, arguments) {
            Ok(result) => McpToolResponse::success(result),
            Err(e) => McpToolResponse::error(format!("{}", e)),
        }
    }
}

// ============================================================================
// 宏辅助：为 #[tool] 类型生成 McpServer 实现
// ============================================================================

/// 为类型生成 McpServer 实现的宏
///
/// 此宏由 `#[tool]` 宏内部自动调用，用户无需手动使用。
#[macro_export]
macro_rules! impl_mcp_server {
    ($type:ty) => {
        impl $type {
            /// 创建 MCP 服务器实例
            #[cfg(feature = "mcp")]
            pub fn new_mcp_server() -> $crate::mcp::McpServerWrapper<Self> {
                $crate::mcp::McpServerWrapper::new(Self::default())
            }

            /// 获取 MCP 工具定义列表
            #[cfg(feature = "mcp")]
            pub fn mcp_tool_definitions() -> Vec<$crate::mcp::McpTool> {
                $crate::mcp::to_mcp_tools(<Self as $crate::ToolProvider>::tool_definitions())
            }
        }
    };
}

// ============================================================================
// MCP HTTP 服务器（可选功能，需要 http-server feature）
// ============================================================================

/// MCP HTTP 服务器配置
#[cfg(feature = "http-server")]
#[derive(Debug, Clone)]
pub struct McpHttpConfig {
    /// 监听地址
    pub host: String,
    /// 监听端口
    pub port: u16,
    /// 是否启用 CORS
    pub cors_enabled: bool,
}

#[cfg(feature = "http-server")]
impl Default for McpHttpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            cors_enabled: true,
        }
    }
}

/// MCP HTTP 服务器
///
/// 提供基于 HTTP 的 MCP 协议实现。
///
/// # 示例
///
/// ```rust,ignore
/// use tokitai::{tool, mcp::McpHttpServer};
///
/// #[tool]
/// struct Calculator;
///
/// #[tool]
/// impl Calculator {
///     pub fn add(&self, a: i32, b: i32) -> i32 {
///         a + b
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let server = McpHttpServer::new(Calculator::default());
///     server.run("127.0.0.1:8080").await.unwrap();
/// }
/// ```
#[cfg(feature = "http-server")]
pub struct McpHttpServer<T> {
    #[allow(dead_code)]
    inner: T,
    #[allow(dead_code)]
    config: McpHttpConfig,
}

#[cfg(feature = "http-server")]
impl<T> McpHttpServer<T>
where
    T: tokitai_core::ToolProvider + Clone + Send + Sync + 'static,
{
    /// 创建新的 MCP HTTP 服务器
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            config: McpHttpConfig::default(),
        }
    }

    /// 创建带配置的服务器
    pub fn with_config(inner: T, config: McpHttpConfig) -> Self {
        Self { inner, config }
    }

    /// 运行服务器
    ///
    /// # 参数
    ///
    /// - `addr` - 监听地址，格式为 "host:port"
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// server.run("127.0.0.1:8080").await?;
    /// ```
    pub async fn run(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        use axum::{extract::State, routing::get, Json, Router};
        use std::sync::Arc;

        let addr_parts: Vec<&str> = addr.split(':').collect();
        let host = addr_parts.first().unwrap_or(&"127.0.0.1");
        let port: u16 = addr_parts
            .get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        let app_state = Arc::new(AppState::new(to_mcp_tools(T::tool_definitions())));

        // 定义处理器
        async fn list_tools_handler(State(state): State<Arc<AppState>>) -> Json<Vec<McpTool>> {
            Json(state.tools.clone())
        }

        async fn call_tool_handler(Json(_request): Json<McpToolCall>) -> Json<McpToolResponse> {
            // 在实际应用中，这里会调用具体的工具实现
            Json(McpToolResponse::error("Tool execution requires concrete implementation. Use examples/mcp_http_server.rs for a complete example."))
        }

        async fn health_handler() -> &'static str {
            "OK"
        }

        let app = Router::new()
            .route("/tools", get(list_tools_handler))
            .route("/call", axum::routing::post(call_tool_handler))
            .route("/health", get(health_handler))
            .with_state(app_state);

        let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port)).await?;
        println!("MCP Server listening on http://{}:{}", host, port);
        println!("  - GET  /tools   - List available tools");
        println!("  - POST /call    - Call a tool");
        println!("  - GET  /health  - Health check");

        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// 应用状态（用于独立 HTTP 服务器）
#[cfg(feature = "http-server")]
pub struct AppState {
    pub tools: Vec<McpTool>,
}

#[cfg(feature = "http-server")]
impl AppState {
    /// 创建新的应用状态
    pub fn new(tools: Vec<McpTool>) -> Self {
        Self { tools }
    }
}

// ============================================================================
// SSE (Server-Sent Events) 支持
// ============================================================================

/// MCP SSE 消息
#[cfg(all(feature = "http-server", feature = "runtime"))]
#[derive(Debug, Clone, Serialize)]
pub struct McpSseMessage {
    pub event: String,
    pub data: serde_json::Value,
}

#[cfg(all(feature = "http-server", feature = "runtime"))]
impl McpSseMessage {
    pub fn tool_list(tools: Vec<McpTool>) -> Self {
        Self {
            event: "tools/list".to_string(),
            data: serde_json::to_value(tools).unwrap_or_default(),
        }
    }

    pub fn tool_result(result: McpToolResponse) -> Self {
        Self {
            event: "tool/result".to_string(),
            data: serde_json::to_value(result).unwrap_or_default(),
        }
    }
}
