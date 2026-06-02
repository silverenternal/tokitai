//! MCP (Model Context Protocol) support
//!
//! Provides an MCP-compatible tool definition format and server runtime.
//!
//! # Quick Start
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
//! // Create an MCP server
//! let server = Calculator::new_mcp_server();
//! ```

use serde::{Deserialize, Serialize};
use tokitai_core::ToolDefinition;

#[cfg(feature = "mcp")]
use async_trait::async_trait;

/// MCP tool definition format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    /// Output schema (advertises the return type of the tool). Populated by
    /// `to_mcp_tools` when the tool's return type has a `#[tool_type]` schema
    /// registered in the global `TYPE_SCHEMA_CACHE`. `None` for tools whose
    /// return type is not registered (e.g. primitives, plain `serde_json::Value`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
}

/// Converts tokitai tool definitions to MCP format
pub fn to_mcp_tools(tools: &[ToolDefinition]) -> Vec<McpTool> {
    tools
        .iter()
        .filter_map(|t| match serde_json::from_str(&t.input_schema) {
            Ok(schema) => Some(McpTool {
                name: t.name.to_string(),
                description: t.description.to_string(),
                input_schema: schema,
                output_schema: None,
            }),
            Err(e) => {
                log::warn!("failed to parse schema for tool '{}': {}", t.name, e);
                None
            }
        })
        .collect()
}

/// MCP tool call request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// MCP tool call response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl McpToolResponse {
    /// Build a successful response wrapping `result`.
    ///
    /// Sets `success: true`, stores `result` in the `result` field, and
    /// leaves `error` as `None`.
    pub fn success(result: serde_json::Value) -> Self {
        Self {
            success: true,
            result: Some(result),
            error: None,
        }
    }

    /// Build a failure response with the given error `message`.
    ///
    /// Sets `success: false`, stores the message in the `error` field, and
    /// leaves `result` as `None`.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(message.into()),
        }
    }
}

// ============================================================================
// MCP Server Trait - core abstraction
// ============================================================================

/// MCP server trait
///
/// Provides the tool list and call interface required by the MCP protocol.
/// This trait is automatically implemented for all `#[tool]` types and does
/// not need to be implemented manually.
///
/// # Examples
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
/// // McpServer is automatically implemented
/// let server = MyTools::new_mcp_server();
/// let tools = server.list_tools().await;
/// let result = server.call_tool("greet", &json!({"name": "Alice"})).await;
/// ```
#[cfg(feature = "mcp")]
#[async_trait]
pub trait McpServer: Sized + Send + Sync {
    /// Returns all available tool definitions
    async fn list_tools(&self) -> Vec<McpTool>;

    /// Invokes the specified tool
    async fn call_tool(&self, name: &str, arguments: &serde_json::Value) -> McpToolResponse;

    /// Returns the number of tools
    async fn tool_count(&self) -> usize {
        self.list_tools().await.len()
    }
}

// ============================================================================
// Auto-impl McpServer for all #[tool] types
// ============================================================================

/// MCP server wrapper
///
/// Wraps any type implementing `ToolProvider` and `ToolCaller` as an MCP
/// server. This is the foundation on which the `#[tool]` macro automatically
/// implements `McpServer`.
#[cfg(feature = "mcp")]
pub struct McpServerWrapper<T> {
    inner: T,
}

#[cfg(feature = "mcp")]
impl<T> McpServerWrapper<T>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Clone + Send + Sync + 'static,
{
    /// Creates a new MCP server wrapper
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Returns the inner tool instance
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Converts to MCP tool definitions
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
        // Use the ToolCaller trait's call_tool method
        match self.inner.call_tool(name, arguments) {
            Ok(result) => McpToolResponse::success(result),
            Err(e) => McpToolResponse::error(format!("{}", e)),
        }
    }
}

// ============================================================================
// Macro helper: generate McpServer impl for #[tool] types
// ============================================================================

/// Macro that generates an `McpServer` implementation for a type
///
/// This macro is invoked automatically by the `#[tool]` macro and does not
/// need to be called manually by users.
#[macro_export]
macro_rules! impl_mcp_server {
    ($type:ty) => {
        impl $type {
            /// Creates a new MCP server instance
            #[cfg(feature = "mcp")]
            pub fn new_mcp_server() -> $crate::mcp::McpServerWrapper<Self> {
                $crate::mcp::McpServerWrapper::new(Self::default())
            }

            /// Returns the list of MCP tool definitions
            #[cfg(feature = "mcp")]
            pub fn mcp_tool_definitions() -> Vec<$crate::mcp::McpTool> {
                $crate::mcp::to_mcp_tools(<Self as $crate::ToolProvider>::tool_definitions())
            }
        }
    };
}

// ============================================================================
// MCP HTTP server (optional, requires the http-server feature)
// ============================================================================

/// MCP HTTP server configuration
#[cfg(feature = "http-server")]
#[derive(Debug, Clone)]
pub struct McpHttpConfig {
    /// The listen address
    pub host: String,
    /// The listen port
    pub port: u16,
    /// Whether CORS is enabled
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

/// MCP HTTP server
///
/// Provides an HTTP-based MCP protocol implementation.
///
/// # Examples
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
    /// Creates a new MCP HTTP server
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            config: McpHttpConfig::default(),
        }
    }

    /// Creates a server with the given configuration
    pub fn with_config(inner: T, config: McpHttpConfig) -> Self {
        Self { inner, config }
    }

    /// Runs the server
    ///
    /// # Parameters
    ///
    /// - `addr` - the listen address, in "host:port" format
    ///
    /// # Examples
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

        // Define handlers
        async fn list_tools_handler(State(state): State<Arc<AppState>>) -> Json<Vec<McpTool>> {
            Json(state.tools.clone())
        }

        async fn call_tool_handler(Json(_request): Json<McpToolCall>) -> Json<McpToolResponse> {
            // In a real application, this would call the concrete tool implementation
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

/// Application state (used by the standalone HTTP server)
#[cfg(feature = "http-server")]
pub struct AppState {
    pub tools: Vec<McpTool>,
}

#[cfg(feature = "http-server")]
impl AppState {
    /// Creates a new application state
    pub fn new(tools: Vec<McpTool>) -> Self {
        Self { tools }
    }
}

// ============================================================================
// SSE (Server-Sent Events) support
// ============================================================================

/// MCP SSE message
#[cfg(all(feature = "http-server", feature = "runtime"))]
#[derive(Debug, Clone, Serialize)]
pub struct McpSseMessage {
    pub event: String,
    pub data: serde_json::Value,
}

#[cfg(all(feature = "http-server", feature = "runtime"))]
impl McpSseMessage {
    /// Build an SSE message advertising a `tools/list` event whose payload is
    /// the serialized `tools`. Used by the HTTP transport to push a fresh
    /// tool catalogue to subscribers.
    pub fn tool_list(tools: Vec<McpTool>) -> Self {
        Self {
            event: "tools/list".to_string(),
            data: serde_json::to_value(tools).unwrap_or_default(),
        }
    }

    /// Build an SSE message advertising a `tool/result` event whose payload
    /// is the serialized `result`. Used by the HTTP transport to push a
    /// tool call outcome to subscribers.
    pub fn tool_result(result: McpToolResponse) -> Self {
        Self {
            event: "tool/result".to_string(),
            data: serde_json::to_value(result).unwrap_or_default(),
        }
    }
}
