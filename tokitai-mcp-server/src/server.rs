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
// T-010: bring `visible_tools` / `call_for_tenant` into scope so the
// dynamic-registry dispatch path resolves trait methods at the use site.
#[allow(unused_imports)]
use tokitai_core::DynamicToolProvider;
// T-021: when the `mcp-typed` feature is enabled, validate caller
// arguments against the matching fixture's `inputSchema` BEFORE
// dispatching to the handler. The typed-dispatch path is intentionally
// `#[allow(dead_code)]` when the feature is off: the import + the
// cached `TypedDispatcher` cost zero cycles (the cache is built lazily
// and only read inside the `#[cfg(feature = "mcp-typed")]` branch).
#[cfg(feature = "mcp-typed")]
use crate::typed::{load_typed_fixtures, TypedDispatcher, TypedToolSpec};
#[cfg(feature = "mcp-typed")]
use std::collections::HashSet;
#[cfg(feature = "mcp-typed")]
use std::sync::{Mutex, OnceLock};

// T-021: lazily build a process-wide `TypedDispatcher` from the fixture
// directory. The cache survives the lifetime of the process so repeated
// `tools/call` requests do not pay the file-system scan cost. The cache
// is built the first time a typed request arrives and is then read-only.
#[cfg(feature = "mcp-typed")]
fn typed_dispatcher() -> &'static TypedDispatcher {
    static CACHE: OnceLock<TypedDispatcher> = OnceLock::new();
    CACHE.get_or_init(|| {
        let specs: Vec<TypedToolSpec> = load_typed_fixtures();
        TypedDispatcher::from_specs(specs)
    })
}

// T-021: per-tool warn-once log for tools that have no matching
// `tests/fixtures/mcp-spec/typed/*.json` fixture. Without this
// signal an operator who adds a new tool but forgets to drop a
// fixture alongside it would silently lose typed validation coverage
// (the call goes through the T-005 JSON-passthrough path). The
// `OnceLock<Mutex<HashSet<String>>>` keeps the de-duplication cost
// O(1) per call after the first hit, and the Mutex is only held
// for the HashSet insert — never around the warn! call — so the
// hot path is uncontended.
//
// Split into a pure `record_missing_fixture` (returns whether this
// is the first time we have seen `tool_name`) and a thin wrapper
// that emits the `warn!` only when the pure function says "first
// time". Splitting the function this way lets the unit test verify
// the dedup logic without depending on a tracing subscriber, which
// would require pulling in a new test dependency and is forbidden
// by the no-new-deps rule.
#[cfg(feature = "mcp-typed")]
fn record_missing_fixture(tool_name: &str) -> bool {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let cell = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = cell.lock().expect("warn-missing-fixture mutex poisoned");
    guard.insert(tool_name.to_string())
}

#[cfg(feature = "mcp-typed")]
fn warn_missing_fixture_once(tool_name: &str) {
    if record_missing_fixture(tool_name) {
        warn!(
            "T-021 no typed spec for tool `{}`; dispatching unvalidated \
             (add a fixture under tests/fixtures/mcp-spec/typed/ to enable \
             validation). This warning fires once per missing tool name.",
            tool_name
        );
    }
}
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

    #[allow(dead_code)] // used by read-only McpServer paths (kept for API symmetry)
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

    /// Switch to the stdio MCP transport. Returns a builder that can be
    /// used to run a JSON-RPC-over-stdio MCP server using the same
    /// `ToolProvider` configured above.
    ///
    /// The HTTP / CORS / tracing settings on `McpServerConfig` are
    /// irrelevant for the stdio transport and are dropped.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use tokitai_mcp_server::McpServerBuilder;
    ///
    /// let provider = MyTools::default();
    /// let stdio = McpServerBuilder::with_tool(provider).with_stdio().build();
    /// stdio.serve_stdio().await?;
    /// ```
    pub fn with_stdio(self) -> crate::stdio::McpServerStdioBuilder<T> {
        crate::stdio::McpServerStdioBuilder::from_arc(Arc::new(self.tool_provider))
    }
}

/// Helper function to get tool definitions from a provider
/// Works with both regular providers (via static method), `MultiToolProvider`
/// (via instance method), and `DynamicToolRegistry` (T-010).
fn get_tools_from_provider<T>(provider: &T) -> Vec<mcp::McpTool>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    // First try static method (works for most providers)
    let static_tools = T::tool_definitions();
    if !static_tools.is_empty() {
        return mcp::to_mcp_tools(static_tools);
    }

    // If static method returns empty, try to get from instance (for MultiToolProvider
    // or DynamicToolRegistry). Type-based dispatch avoids forcing every T to
    // implement a no-op `runtime_tool_definitions` method.
    get_tools_from_provider_runtime(provider)
}

/// Runtime check for providers with dynamic tool definitions
///
/// # Design Note: Why Type-Based Dispatch?
///
/// This function uses type-based dispatch to handle `MultiToolProvider` and
/// `DynamicToolRegistry` (T-010) specially. Both collect tools at runtime,
/// while other providers use compile-time static methods
/// (`ToolProvider::tool_definitions()`).
///
/// The type-based approach avoids introducing a trait that would only have
/// implementations on those two runtime types. If you need a custom provider
/// with runtime tool definitions, consider:
/// 1. Using `MultiToolProvider` to combine your tools
/// 2. Wrapping a `DynamicToolRegistry` for fine-grained per-tenant control
/// 3. Filing an issue to discuss adding a `RuntimeToolProvider` trait
fn get_tools_from_provider_runtime<T>(provider: &T) -> Vec<mcp::McpTool>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    multi_provider_tool_defs(provider)
        .or_else(|| dynamic_registry_tool_defs(provider))
        .unwrap_or_default()
}

/// Return the tool definitions held by a `MultiToolProvider`, or `None`
/// if `provider` is not a `MultiToolProvider` instance. Used by both
/// the HTTP server (via [`get_tools_from_provider_runtime`]) and the
/// stdio transport.
pub(crate) fn multi_provider_tool_defs<T>(provider: &T) -> Option<Vec<mcp::McpTool>>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    use std::any::Any;
    let multi = (provider as &dyn Any).downcast_ref::<MultiToolProvider>()?;
    Some(multi.tool_definitions().to_vec())
}

/// T-010: return the tool definitions held by a `DynamicToolRegistry`,
/// or `None` if `provider` is not one. Used by both the HTTP server and
/// the stdio transport so a dynamic registry plugs into the same
/// `McpServerBuilder::with_tool(...)` entry point as a static provider.
pub(crate) fn dynamic_registry_tool_defs<T>(provider: &T) -> Option<Vec<mcp::McpTool>>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    use std::any::Any;
    let reg = (provider as &dyn Any).downcast_ref::<tokitai_core::DynamicToolRegistry>()?;
    Some(
        reg.visible_tools(None)
            .iter()
            .map(|def| mcp::McpTool {
                name: def.name.clone(),
                description: def.description.clone(),
                input_schema: serde_json::from_str(&def.input_schema)
                    .unwrap_or_else(|_| serde_json::json!({})),
                output_schema: None,
            })
            .collect(),
    )
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

        // T-010: the registry uses an interior-mutability pattern so the
        // dynamic provider's tool list can be refreshed on every request.
        // For static providers, this is a no-op cache.
        let registry = Arc::new(LiveToolRegistry::new(
            self.tools.clone(),
            self.tool_provider.clone(),
        ));

        let state = Arc::new(AppStateWithProvider {
            registry,
            tool_provider: self.tool_provider.clone(),
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

    /// Get the list of tools. T-010: when the provider is a
    /// `DynamicToolRegistry`, this reflects the current (post-mutation)
    /// state of the registry; for static providers it returns the
    /// cached slice captured at build time.
    pub fn tools(&self) -> Vec<mcp::McpTool> {
        // Re-fetch from the provider each call so dynamic mutation is
        // visible. For static providers this is `O(n)` and just returns
        // the same data they baked in at compile time.
        get_tools_from_provider(&*self.tool_provider)
    }

    /// Get the tool provider
    pub fn tool_provider(&self) -> &T {
        &self.tool_provider
    }
}

/// T-010: interior-mutable registry. Holds the cached snapshot of the
/// tool list AND a reference to the live provider; `tools()` re-fetches
/// from the provider on every call so dynamic mutation is honoured.
/// For static providers, the re-fetch is a cheap `O(n)` walk that
/// returns the same `&'static [ToolDefinition]` they baked at compile
/// time, so the cost is negligible.
struct LiveToolRegistry {
    cached: std::sync::RwLock<Vec<mcp::McpTool>>,
    provider: Arc<dyn std::any::Any + Send + Sync>,
}

impl LiveToolRegistry {
    fn new<T>(initial: Vec<mcp::McpTool>, provider: Arc<T>) -> Self
    where
        T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
    {
        Self {
            cached: std::sync::RwLock::new(initial),
            provider: provider as Arc<dyn std::any::Any + Send + Sync>,
        }
    }

    /// Refresh the cached tool list from the live provider and return
    /// a clone of the freshly-fetched list. Falls back to the cached
    /// list when the refresh fails (e.g. poisoned lock).
    fn tools(&self) -> Vec<mcp::McpTool> {
        // Try to refresh from a `DynamicToolRegistry`. For any other
        // provider type we keep the cached slice, which matches the
        // compile-time static semantics.
        if let Some(reg) = self
            .provider
            .downcast_ref::<tokitai_core::DynamicToolRegistry>()
        {
            let fresh: Vec<mcp::McpTool> =
                <tokitai_core::DynamicToolRegistry as tokitai_core::DynamicToolProvider>::visible_tools(
                    reg, None,
                )
                .iter()
                .map(|def| mcp::McpTool {
                    name: def.name.clone(),
                    description: def.description.clone(),
                    input_schema: serde_json::from_str(&def.input_schema)
                        .unwrap_or_else(|_| serde_json::json!({})),
                    output_schema: None,
                })
                .collect();
            if let Ok(mut guard) = self.cached.write() {
                *guard = fresh.clone();
            }
            return fresh;
        }
        // Non-dynamic providers: hand back the cached slice.
        self.cached.read().map(|g| g.clone()).unwrap_or_default()
    }

    fn find(&self, name: &str) -> Option<mcp::McpTool> {
        self.tools().into_iter().find(|t| t.name == name)
    }
}

/// Application state with provider
struct AppStateWithProvider<T> {
    registry: Arc<LiveToolRegistry>,
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
    Json(state.registry.tools())
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

    // Find the tool. The live registry refreshes its tool list when
    // the provider is a `DynamicToolRegistry`, so a tool added since
    // server start is reachable here.
    let tool = state.registry.find(&request.name).ok_or_else(|| {
        warn!("Tool not found: {}", request.name);
        StatusCode::NOT_FOUND
    })?;

    info!("Found tool: {} - {}", tool.name, tool.description);

    // T-021: when the `mcp-typed` feature is on, validate the caller's
    // arguments against the matching fixture's `inputSchema` BEFORE the
    // handler runs. The handler is unreachable from a malformed call.
    // On a validation failure we return 400 Bad Request (not 500) with
    // the JSON Pointer to the offending field embedded in the response
    // body, so the LLM client can correct its input and retry.
    //
    // When the request targets a tool with no matching fixture we
    // still fall through to the T-005 JSON-passthrough path (the
    // fixture set is a project-supplied artefact, not a complete
    // registry of every tool the provider exposes), but we emit a
    // warn! exactly once per such tool name so an operator can see
    // coverage gaps instead of discovering them after a security
    // incident. See `warn_missing_fixture_once` for the dedup
    // mechanism.
    #[cfg(feature = "mcp-typed")]
    {
        let dispatcher = typed_dispatcher();
        match dispatcher.find(&request.name) {
            Some(spec) => {
                if let Err(e) = spec.validate(&request.arguments) {
                    warn!(
                        "T-021 typed validation refused call: name={} err={}",
                        request.name, e
                    );
                    return Ok(Json(ToolCallResponse::error(format!("{}", e))));
                }
            }
            None => {
                warn_missing_fixture_once(&request.name);
            }
        }
    }

    // T-010: when the provider is a `DynamicToolRegistry`, dispatch
    // through `call_for_tenant` so per-tenant enable/disable is honored.
    // We downcast on the Arc-wrapped provider so the registry lives in
    // exactly one place and the dispatch logic stays out of the
    // generic T-bound.
    if let Some(reg) = dynamic_registry_from_state(&state) {
        match reg.call_for_tenant(&request.name, None, &request.arguments) {
            Ok(result) => {
                info!("Tool executed successfully: {}", request.name);
                return Ok(Json(ToolCallResponse::success(result)));
            }
            Err(e) => {
                warn!("Tool execution failed: {} - {}", request.name, e);
                return Ok(Json(ToolCallResponse::error(format!("{}", e))));
            }
        }
    }

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

/// T-010: downcast the wrapped provider to `DynamicToolRegistry` so the
/// handler can call `call_for_tenant` instead of the generic
/// `ToolCaller::call_tool`. Returns `None` when the wrapped provider is
/// not a dynamic registry.
fn dynamic_registry_from_state<T>(
    state: &AppStateWithProvider<T>,
) -> Option<tokitai_core::DynamicToolRegistry>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    use std::any::Any;
    (state.tool_provider.as_ref() as &dyn Any)
        .downcast_ref::<tokitai_core::DynamicToolRegistry>()
        .cloned()
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
                output_schema: None,
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

// =====================================================================
// T-021 fail-closed state-drift visibility test.
//
// The state-drift failure mode is: an operator adds a new tool to
// the provider, but forgets to drop a matching fixture in
// `tests/fixtures/mcp-spec/typed/`. Without a signal, the call goes
// through the T-005 JSON-passthrough path silently and the operator
// learns about the missing coverage only after a security incident.
//
// `record_missing_fixture` is the dedup state machine behind
// `warn_missing_fixture_once`. We test the state machine directly
// (the warn! call is a thin shim around it) because pulling in a
// tracing-test subscriber would be a new dev-dependency and the
// project rule is "no new top-level dependencies". The test uses
// a unique tool name per invocation so the `OnceLock<HashSet>`
// global does not collide with a previous test run's state.
// =====================================================================
#[cfg(all(test, feature = "mcp-typed"))]
mod warn_missing_fixture_tests {
    use super::record_missing_fixture;

    #[test]
    fn first_call_for_a_tool_name_returns_true() {
        // Unique name per test run so we are not at the mercy of
        // test parallelism / ordering.
        let name = format!("__test_first_call_{}__", std::process::id());
        assert!(
            record_missing_fixture(&name),
            "first call for a fresh tool name must report 'first time'"
        );
    }

    #[test]
    fn second_call_for_same_tool_name_returns_false() {
        let name = format!("__test_second_call_{}__", std::process::id());
        assert!(
            record_missing_fixture(&name),
            "first call reports 'first time'"
        );
        assert!(
            !record_missing_fixture(&name),
            "second call for the same tool name must report 'already seen'"
        );
    }

    #[test]
    fn distinct_tool_names_do_not_collide() {
        let name_a = format!("__test_distinct_a_{}__", std::process::id());
        let name_b = format!("__test_distinct_b_{}__", std::process::id());
        assert!(record_missing_fixture(&name_a));
        assert!(
            record_missing_fixture(&name_b),
            "a fresh name must report 'first time' even after another name was inserted"
        );
    }
}
