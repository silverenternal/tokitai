//! MCP Server implementation

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, sync::Arc};
use tokitai::mcp;
use tokitai_core::serde_types;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info, warn};

/// Server error types
#[derive(Debug)]
pub enum ServerError {
    /// Tool not found
    ToolNotFound(String),
    /// Tool execution failed
    ToolExecutionError(String),
    /// Invalid arguments
    InvalidArguments(String),
    /// Server startup failed
    ServerStartupError(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::ToolNotFound(name) => write!(f, "Tool not found: {}", name),
            ServerError::ToolExecutionError(msg) => write!(f, "Tool execution error: {}", msg),
            ServerError::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            ServerError::ServerStartupError(msg) => write!(f, "Server startup error: {}", msg),
        }
    }
}

impl Error for ServerError {}

impl From<Box<dyn Error>> for ServerError {
    fn from(err: Box<dyn Error>) -> Self {
        ServerError::ToolExecutionError(err.to_string())
    }
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Host address to bind to
    pub host: String,
    /// Port to listen on
    pub port: u16,
    /// Enable CORS
    pub cors_enabled: bool,
    /// Enable request tracing
    pub tracing_enabled: bool,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            cors_enabled: true,
            tracing_enabled: true,
        }
    }
}

impl McpServerConfig {
    /// Create a new configuration with custom host and port
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            ..Default::default()
        }
    }

    /// Set CORS enabled
    pub fn with_cors(mut self, enabled: bool) -> Self {
        self.cors_enabled = enabled;
        self
    }

    /// Set tracing enabled
    pub fn with_tracing(mut self, enabled: bool) -> Self {
        self.tracing_enabled = enabled;
        self
    }

    /// Get the full address string
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Tool call request
#[derive(Debug, Deserialize)]
pub struct ToolCallRequest {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Tool call response
#[derive(Debug, Serialize)]
pub struct ToolCallResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolCallResponse {
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

/// Internal tool registry
struct ToolRegistry {
    tools: Vec<mcp::McpTool>,
}

impl ToolRegistry {
    fn new(tools: Vec<mcp::McpTool>) -> Self {
        Self { tools }
    }

    fn find(&self, name: &str) -> Option<&mcp::McpTool> {
        self.tools.iter().find(|t| t.name == name)
    }
}

/// Application state
struct AppState {
    registry: ToolRegistry,
}

// ============================================================================
// Generic MCP Server Builder
// ============================================================================

/// MCP Server builder with generic type support
///
/// # Example
///
/// ```rust,ignore
/// use tokitai_mcp_server::McpServerBuilder;
/// use tokitai::tool;
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
///     let server = McpServerBuilder::with_tool(Calculator::default())
///         .with_port(3000)
///         .build();
///
///     server.run().await.unwrap();
/// }
/// ```
pub struct McpServerBuilder<T> {
    config: McpServerConfig,
    tool_provider: T,
}

impl<T> McpServerBuilder<T>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    /// Create a new server builder with a tool provider
    pub fn with_tool(tool: T) -> Self {
        Self {
            config: McpServerConfig::default(),
            tool_provider: tool,
        }
    }

    /// Create a new server builder with configuration
    pub fn with_config(config: McpServerConfig, tool: T) -> Self {
        Self {
            config,
            tool_provider: tool,
        }
    }

    /// Set the server port
    pub fn with_port(mut self, port: u16) -> Self {
        self.config.port = port;
        self
    }

    /// Set the server host
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.config.host = host.into();
        self
    }

    /// Enable or disable CORS
    pub fn with_cors(mut self, enabled: bool) -> Self {
        self.config.cors_enabled = enabled;
        self
    }

    /// Enable or disable tracing
    pub fn with_tracing(mut self, enabled: bool) -> Self {
        self.config.tracing_enabled = enabled;
        self
    }

    /// Build the server
    pub fn build(self) -> McpServerWithProvider<T>
    where
        T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
    {
        // Try to get tool definitions from the provider instance first (for MultiToolProvider),
        // then fall back to static method
        let tools = get_tools_from_provider(&self.tool_provider);
        McpServerWithProvider {
            config: self.config,
            tool_provider: Arc::new(self.tool_provider),
            tools,
        }
    }
}

/// Helper function to get tool definitions from a provider
/// Works with both regular providers (via static method) and MultiToolProvider (via instance method)
fn get_tools_from_provider<T>(provider: &T) -> Vec<mcp::McpTool>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    // First try static method (works for most providers)
    let static_tools = T::tool_definitions();
    if !static_tools.is_empty() {
        return mcp::to_mcp_tools(static_tools);
    }

    // If static method returns empty, try to get from instance (for MultiToolProvider)
    // This is a special case for providers that collect tools at runtime
    // We use the RuntimeToolProvider trait
    get_tools_from_provider_runtime(provider)
}

/// Runtime check for providers with dynamic tool definitions
///
/// # Design Note: Why Type-Based Dispatch?
///
/// This function uses type-based dispatch to handle `MultiToolProvider` specially.
/// This is a deliberate design choice: `MultiToolProvider` collects tools at runtime,
/// while other providers use compile-time static methods (`ToolProvider::tool_definitions()`).
///
/// The type-based approach avoids introducing a trait that would only have a single implementation.
/// If you need a custom provider with runtime tool definitions, consider:
/// 1. Using `MultiToolProvider` to combine your tools
/// 2. Filing an issue to discuss adding a `RuntimeToolProvider` trait
fn get_tools_from_provider_runtime<T>(provider: &T) -> Vec<mcp::McpTool>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    use std::any::Any;
    if let Some(multi) = (provider as &dyn Any).downcast_ref::<MultiToolProvider>() {
        return multi.tool_definitions().to_vec();
    }

    Vec::new()
}

/// MCP Server with a concrete tool provider
pub struct McpServerWithProvider<T> {
    config: McpServerConfig,
    tool_provider: Arc<T>,
    tools: Vec<mcp::McpTool>,
}

impl<T> McpServerWithProvider<T>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    /// Create a new MCP server with a tool provider
    pub fn new(config: McpServerConfig, tool_provider: T) -> Self {
        let tools = get_tools_from_provider(&tool_provider);
        Self {
            config,
            tool_provider: Arc::new(tool_provider),
            tools,
        }
    }

    /// Run the server with default address
    pub async fn run(&self) -> Result<(), ServerError> {
        self.run_with_address(&self.config.address()).await
    }

    /// Run the server with a specific address
    ///
    /// # Arguments
    ///
    /// * `addr` - Address in format "host:port"
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// server.run_with_address("0.0.0.0:3000").await?;
    /// ```
    pub async fn run_with_address(&self, addr: &str) -> Result<(), ServerError> {
        // Initialize tracing (only if not already set)
        if self.config.tracing_enabled && !tracing::dispatcher::has_been_set() {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive("tokitai_mcp_server=info".parse().unwrap()),
                )
                .init();
        }

        let state = Arc::new(AppStateWithProvider {
            registry: ToolRegistry::new(self.tools.clone()),
            tool_provider: self.tool_provider.clone(), // 现在只是克隆 Arc，不是克隆 T
        });

        // Build router
        let mut app = Router::new()
            .route("/tools", get(list_tools_handler_with_provider))
            .route("/call", post(call_tool_handler_with_provider))
            .route("/health", get(health_handler))
            .with_state(state);

        // Add CORS if enabled
        if self.config.cors_enabled {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            app = app.layer(cors);
        }

        // Add tracing if enabled
        if self.config.tracing_enabled {
            app = app.layer(TraceLayer::new_for_http());
        }

        info!("Starting MCP server on http://{}", addr);
        info!("Endpoints:");
        info!("  GET  /tools  - List available tools");
        info!("  POST /call   - Call a tool");
        info!("  GET  /health - Health check");

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| ServerError::ServerStartupError(e.to_string()))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| ServerError::ServerStartupError(e.to_string()))?;

        Ok(())
    }

    /// Get the server configuration
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// Get the list of tools
    pub fn tools(&self) -> &[mcp::McpTool] {
        &self.tools
    }

    /// Get the tool provider
    pub fn tool_provider(&self) -> &T {
        &self.tool_provider
    }
}

/// Application state with provider
struct AppStateWithProvider<T> {
    registry: ToolRegistry,
    tool_provider: Arc<T>,
}

/// MCP Server (只读模式 - 不支持工具调用)
///
/// # Limitations
///
/// 此类型仅支持 `/tools` 端点，`/call` 端点返回 `501 Not Implemented`
/// 如需完整功能，请使用 [`McpServerBuilder`] + [`MultiToolProvider`]
///
/// # Example
///
/// ```rust,ignore
/// use tokitai_mcp_server::McpServer;
///
/// #[tokio::main]
/// async fn main() {
///     let server = McpServer::new();
///     server.run().await.unwrap();
/// }
/// ```
pub struct McpServer {
    config: McpServerConfig,
    tools: Vec<mcp::McpTool>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    /// Create a new server with default configuration
    pub fn new() -> Self {
        Self {
            config: McpServerConfig::default(),
            tools: Vec::new(),
        }
    }

    /// Create a new server with custom configuration
    pub fn with_config(config: McpServerConfig) -> Self {
        Self {
            config,
            tools: Vec::new(),
        }
    }

    /// Create a server from tool providers
    pub fn from_tools(tools: Vec<mcp::McpTool>) -> Self {
        Self {
            config: McpServerConfig::default(),
            tools,
        }
    }

    /// Run the server with default address
    pub async fn run(&self) -> Result<(), ServerError> {
        self.run_with_address(&self.config.address()).await
    }

    /// Run the server with a specific address
    ///
    /// # Arguments
    ///
    /// * `addr` - Address in format "host:port"
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// server.run_with_address("0.0.0.0:3000").await?;
    /// ```
    pub async fn run_with_address(&self, addr: &str) -> Result<(), ServerError> {
        // Initialize tracing (only if not already set)
        if self.config.tracing_enabled && !tracing::dispatcher::has_been_set() {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive("tokitai_mcp_server=info".parse().unwrap()),
                )
                .init();
        }

        let state = Arc::new(AppState {
            registry: ToolRegistry::new(self.tools.clone()),
        });

        // Build router
        let mut app = Router::new()
            .route("/tools", get(list_tools_handler))
            .route("/call", post(call_tool_handler))
            .route("/health", get(health_handler))
            .with_state(state);

        // Add CORS if enabled
        if self.config.cors_enabled {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            app = app.layer(cors);
        }

        // Add tracing if enabled
        if self.config.tracing_enabled {
            app = app.layer(TraceLayer::new_for_http());
        }

        info!("Starting MCP server on http://{}", addr);
        info!("Endpoints:");
        info!("  GET  /tools  - List available tools");
        info!("  POST /call   - Call a tool");
        info!("  GET  /health - Health check");

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| ServerError::ServerStartupError(e.to_string()))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| ServerError::ServerStartupError(e.to_string()))?;

        Ok(())
    }

    /// Get the server configuration
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// Get the list of tools
    pub fn tools(&self) -> &[mcp::McpTool] {
        &self.tools
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// List tools handler
async fn list_tools_handler(State(state): State<Arc<AppState>>) -> Json<Vec<mcp::McpTool>> {
    Json(state.registry.tools.clone())
}

/// Call tool handler (without tool provider - returns 501 Not Implemented)
///
/// This handler always returns `501 Not Implemented` because `McpServer`
/// doesn't have a tool provider. Use `McpServerBuilder` with `MultiToolProvider`
/// for full tool call support.
async fn call_tool_handler(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<ToolCallRequest>,
) -> Result<Json<ToolCallResponse>, StatusCode> {
    info!("Tool call request (read-only mode): name={}", request.name);
    // Without a tool provider, we can't execute tools
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// List tools handler with provider
async fn list_tools_handler_with_provider<T>(
    State(state): State<Arc<AppStateWithProvider<T>>>,
) -> Json<Vec<mcp::McpTool>>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    Json(state.registry.tools.clone())
}

/// Call tool handler with provider
async fn call_tool_handler_with_provider<T>(
    State(state): State<Arc<AppStateWithProvider<T>>>,
    Json(request): Json<ToolCallRequest>,
) -> Result<Json<ToolCallResponse>, StatusCode>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    info!(
        "Tool call request: name={}, arguments={:?}",
        request.name, request.arguments
    );

    // Find the tool
    let tool = state.registry.find(&request.name).ok_or_else(|| {
        warn!("Tool not found: {}", request.name);
        StatusCode::NOT_FOUND
    })?;

    info!("Found tool: {} - {}", tool.name, tool.description);

    // Call the actual tool
    match state
        .tool_provider
        .call_tool(&request.name, &request.arguments)
    {
        Ok(result) => {
            info!("Tool executed successfully: {}", request.name);
            Ok(Json(ToolCallResponse::success(result)))
        }
        Err(e) => {
            warn!("Tool execution failed: {} - {}", request.name, e);
            Ok(Json(ToolCallResponse::error(format!("{}", e))))
        }
    }
}

/// Health check handler
async fn health_handler() -> &'static str {
    "OK"
}

// ============================================================================
// Multi-Tool Provider Support
// ============================================================================

/// A provider that combines multiple tool providers into one
///
/// This allows you to register multiple tool types and serve them together.
///
/// # Example
///
/// ```rust,ignore
/// use tokitai_mcp_server::MultiToolProvider;
///
/// let mut provider = MultiToolProvider::new();
/// provider.add(Calculator::default());
/// provider.add(TextTools::default());
///
/// let server = McpServerBuilder::with_tool(provider)
///     .with_port(8080)
///     .build();
/// ```
pub struct MultiToolProvider {
    providers: Vec<Box<dyn ToolCallerDyn>>,
    tool_defs: Vec<mcp::McpTool>,
}

/// Dynamic tool caller trait object for runtime polymorphism
///
/// # Why This Trait Exists
///
/// Rust's type system requires knowing the concrete type at compile time. However,
/// `MultiToolProvider` needs to store multiple different tool types (`Calculator`,
/// `TextTools`, etc.) in a single collection and call them uniformly.
///
/// This trait object (`Box<dyn ToolCallerDyn>`) enables that by:
/// 1. **Erasing the concrete type** - Store any tool provider in a `Vec`
/// 2. **Dynamic dispatch** - Call tools without knowing their types at compile time
/// 3. **Type safety** - Still enforces `Send + Sync` for thread safety
///
/// # How It Works
///
/// The `#[tool]` macro automatically implements this trait for any type that
/// implements both `ToolProvider` and `ToolCaller`. This means you can seamlessly
/// mix compile-time tool definitions with runtime polymorphism.
///
/// # Example
///
/// ```rust,ignore
/// // Behind the scenes, MultiToolProvider does this:
/// let mut providers: Vec<Box<dyn ToolCallerDyn>> = Vec::new();
///
/// // Each tool type is boxed as a trait object
/// providers.push(Box::new(Calculator::default()));
/// providers.push(Box::new(TextTools::default()));
///
/// // Call tools uniformly without knowing concrete types
/// for provider in &providers {
///     provider.call_tool("add", &args)?;
/// }
/// ```
pub trait ToolCallerDyn: Send + Sync {
    /// Call a tool by name
    ///
    /// # Arguments
    ///
    /// * `name` - The tool name to call
    /// * `args` - JSON arguments passed to the tool
    ///
    /// # Returns
    ///
    /// The tool's result as a JSON value, or an error if the tool fails.
    fn call_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, tokitai_core::ToolError>;
}

impl<T> ToolCallerDyn for T
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    fn call_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, tokitai_core::ToolError> {
        self.call_tool(name, args)
    }
}

impl MultiToolProvider {
    /// Create a new empty multi-tool provider
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            tool_defs: Vec::new(),
        }
    }

    /// Add a tool provider to the collection
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut provider = MultiToolProvider::new();
    /// provider.add(Calculator::default());
    /// provider.add(TextTools::default());
    /// ```
    pub fn add<T>(&mut self, tool: T)
    where
        T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
    {
        // Collect tool definitions from this provider
        for def in T::tool_definitions() {
            // Parse the input_schema from JSON string to Value
            let schema: serde_json::Value =
                serde_json::from_str(&def.input_schema).unwrap_or_else(|_| serde_json::json!({}));

            let mcp_tool = mcp::McpTool {
                name: def.name.clone(),
                description: def.description.clone(),
                input_schema: schema,
            };
            self.tool_defs.push(mcp_tool);
        }

        // Add the provider
        self.providers.push(Box::new(tool));
    }

    /// Get all tool definitions
    pub fn tool_definitions(&self) -> &[mcp::McpTool] {
        &self.tool_defs
    }
}

impl Default for MultiToolProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiToolProvider {
    /// Clone only the tool definitions (metadata), not the tool implementations.
    ///
    /// # Why This Method?
    ///
    /// `MultiToolProvider` cannot be fully cloned because it stores trait objects
    /// (`Box<dyn ToolProvider + ToolCaller>`), which are not cloneable. This method
    /// creates a new provider with the same tool **definitions** (names, descriptions,
    /// schemas), but **without** the actual tool implementations.
    ///
    /// # What Gets Cloned?
    ///
    /// - ✅ Tool names, descriptions, and JSON schemas
    /// - ❌ Tool implementations (the actual code that runs when called)
    ///
    /// # Use Cases
    ///
    /// This is useful when you need to:
    /// - Share tool metadata (e.g., for documentation or UI generation)
    /// - Create a "template" provider that others can add implementations to
    ///
    /// For most use cases, you should create a new `MultiToolProvider` and add
    /// fresh instances of your tools.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut provider = MultiToolProvider::new();
    /// provider.add(Calculator::default());
    ///
    /// // Clone only the definitions
    /// let metadata_only = provider.clone_definitions();
    ///
    /// // metadata_only has the same tool schemas, but no implementations
    /// ```
    pub fn clone_definitions(&self) -> Self {
        if !self.tool_defs.is_empty() {
            tracing::debug!(
                "Cloning MultiToolProvider definitions ({} tools). \
                 Note: The cloned instance has no tool implementations - \
                 only metadata (names, descriptions, schemas).",
                self.tool_defs.len()
            );
        }
        Self {
            providers: Vec::new(),
            tool_defs: self.tool_defs.clone(),
        }
    }
}

impl tokitai_core::ToolProvider for MultiToolProvider {
    fn tool_definitions() -> &'static [tokitai_core::ToolDefinition] {
        // This won't work for MultiToolProvider since we need runtime collection
        // We'll return an empty slice - the actual tool definitions come from
        // the tool_defs field, not this static method
        &[]
    }
}

impl tokitai_core::ToolCaller for MultiToolProvider {
    fn call_tool(
        &self,
        name: &str,
        args: &serde_types::Value,
    ) -> Result<serde_types::Value, tokitai_core::ToolError> {
        // Try each provider until one succeeds or all fail
        for provider in &self.providers {
            // Check if this provider might have the tool
            // (We could optimize this by storing a map of tool names to providers)
            match provider.call_tool(name, args) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // Check if it's a NotFound error, if so, try next provider
                    if matches!(e.kind, tokitai_core::ToolErrorKind::NotFound) {
                        continue;
                    }
                    // Other errors, return immediately
                    return Err(e);
                }
            }
        }

        // No provider found
        Err(tokitai_core::ToolError::not_found(format!(
            "Tool '{}' not found in any provider",
            name
        )))
    }
}
