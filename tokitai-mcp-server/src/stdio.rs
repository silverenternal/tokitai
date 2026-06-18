//! MCP stdio transport.
//!
//! Hand-rolled JSON-RPC 2.0 framing per the MCP `2025-06-18` specification.
//! One JSON object per line on stdin, one JSON object per line on stdout.
//! Newline-delimited; no embedded raw newlines inside messages (the MCP
//! spec requires JSON-RPC messages to be serialized as a single line).
//!
//! ## Why hand-rolled
//!
//! We deliberately avoid `rmcp` or any other MCP SDK so the framer can be
//! pinned against the MCP-2025-06-18 spec via the fixture in
//! `tests/fixtures/mcp-spec/`. Re-sync is a manual edit of this file plus
//! the fixture, not a moving dependency.
//!
//! See `docs/MCP_ARCHITECTURE.md` § "Stdio transport" for the re-sync
//! procedure.
//!
//! ## Supported methods
//!
//! The framer implements the subset of MCP methods needed for a minimal
//! but spec-conformant server:
//!
//! - `initialize`
//! - `ping`
//! - `tools/list`
//! - `tools/call`
//! - `notifications/initialized` (client notification, no response)
//! - `notifications/cancelled` (client notification, no response)
//!
//! Unknown methods produce a JSON-RPC `MethodNotFound` error.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokitai::mcp;
// T-010: bring the trait into scope so `call_for_tenant` resolves.
#[allow(unused_imports)]
use tokitai_core::DynamicToolProvider;

// ============================================================================
// JSON-RPC 2.0 envelope
// ============================================================================

/// JSON-RPC 2.0 request envelope (one line, stdin).
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    /// Must be `"2.0"`. Other values are rejected.
    pub jsonrpc: String,
    /// Method name, e.g. `"tools/call"`.
    pub method: String,
    /// Structured parameters. May be any JSON value; for our supported
    /// methods we always expect an object (or null for `ping`/`initialize`
    /// notifications).
    #[serde(default)]
    pub params: Value,
    /// Caller-supplied correlation id; echoed back in the response. May
    /// be a string, number, or null. We do not enforce uniqueness.
    pub id: Value,
}

/// JSON-RPC 2.0 notification (no `id` field, never produces a response).
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC 2.0 success response (one line, stdout).
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub result: Value,
}

/// JSON-RPC 2.0 error response (one line, stdout).
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub error: JsonRpcError,
}

/// A wire-level reply to a JSON-RPC request. Either a success result
/// or an error object — JSON-RPC 2.0 forbids sending both.
#[derive(Debug, Clone)]
pub enum WireResponse {
    Ok(JsonRpcResponse),
    Err(JsonRpcErrorResponse),
}

impl WireResponse {
    /// Serialize to a single line of JSON, the format required by the
    /// MCP stdio transport.
    pub fn to_line(&self) -> String {
        match self {
            WireResponse::Ok(r) => {
                serde_json::to_string(r).expect("JsonRpcResponse is always serializable")
            }
            WireResponse::Err(r) => {
                serde_json::to_string(r).expect("JsonRpcErrorResponse is always serializable")
            }
        }
    }
}

/// JSON-RPC 2.0 error object. We only emit a small, fixed set of error
/// codes — see <https://www.jsonrpc.org/specification#error_object>.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    /// Numeric error code. `-32601` is `MethodNotFound`, `-32602` is
    /// `InvalidParams`, `-32603` is `InternalError`, `-32700` is
    /// `ParseError`, `-32000` is reserved for server-defined errors.
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method),
            data: None,
        }
    }

    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: msg.into(),
            data: None,
        }
    }

    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self {
            code: -32700,
            message: msg.into(),
            data: None,
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: msg.into(),
            data: None,
        }
    }

    pub fn server_error(code: i32, msg: impl Into<String>) -> Self {
        Self {
            code,
            message: msg.into(),
            data: None,
        }
    }
}

// ============================================================================
// Server identity (initialize handshake)
// ============================================================================

/// Server identity advertised in the `initialize` response.
///
/// Pinned to `2025-06-18` per the MCP spec fixture in
/// `tests/fixtures/mcp-spec/server-identity.json`.
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

/// Server name advertised in the `initialize` response.
pub const MCP_SERVER_NAME: &str = "tokitai-mcp-server";

/// Server version advertised in the `initialize` response.
pub const MCP_SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// Stdio server
// ============================================================================

/// A minimal MCP stdio server.
///
/// Reads JSON-RPC frames one-per-line from `R` (stdin by default) and
/// writes JSON-RPC responses one-per-line to `W` (stdout by default).
/// Both `R` and `W` are generic over `tokio::io::AsyncRead` / `AsyncWrite`
/// for testability — production code uses the `stdin()` / `stdout()`
/// constructors below.
///
/// # Example
///
/// ```rust,ignore
/// use tokitai_mcp_server::StdioServer;
/// use tokitai_mcp_server::MultiToolProvider;
///
/// let provider = MultiToolProvider::default();
/// let server = StdioServer::new(provider);
/// server.serve_stdio().await?;
/// ```
pub struct StdioServer<T> {
    pub(crate) provider: Arc<T>,
}

impl<T> StdioServer<T>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    /// Build a new stdio server wrapping the given provider.
    ///
    /// For `MultiToolProvider` (the most common case for multi-tool
    /// servers), prefer [`StdioServer::new`] with the provider directly.
    pub fn new(provider: T) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }

    /// Get a reference to the wrapped provider.
    pub fn provider(&self) -> &T {
        &self.provider
    }

    /// Run the stdio server using process stdin/stdout. Blocks until EOF
    /// on stdin or an unrecoverable I/O error.
    pub async fn serve_stdio(&self) -> std::io::Result<()> {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        self.serve(stdin, stdout).await
    }

    /// Run the stdio server against arbitrary `AsyncRead` / `AsyncWrite`
    /// streams. Used by tests to inject in-memory pipes.
    pub async fn serve<R, W>(&self, input: R, output: W) -> std::io::Result<()>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let mut reader = BufReader::new(input).lines();
        let mut writer = output;

        while let Some(line) = reader.next_line().await? {
            // Empty lines are not valid JSON-RPC. The MCP spec is silent
            // on this; we choose to skip them so that hand-typed
            // REPL sessions don't blow up on accidental blank lines.
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Try request first (has `id`), then notification (no `id`).
            // If neither matches, return a parse error.
            let response_opt: Option<WireResponse> =
                match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                    Ok(req) => Some(self.handle_request(req).await),
                    Err(_) => match serde_json::from_str::<JsonRpcNotification>(trimmed) {
                        Ok(notif) => {
                            self.handle_notification(notif).await;
                            None
                        }
                        Err(e) => Some(WireResponse::Err(JsonRpcErrorResponse {
                            jsonrpc: "2.0",
                            id: Value::Null,
                            error: JsonRpcError::parse_error(e.to_string()),
                        })),
                    },
                };

            if let Some(resp) = response_opt {
                let serialized = resp.to_line();
                writer.write_all(serialized.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }

        Ok(())
    }

    async fn handle_notification(&self, notif: JsonRpcNotification) {
        if notif.jsonrpc != "2.0" {
            // Spec says malformed notification MUST be ignored — no
            // response. We log a warning in debug builds but otherwise
            // do nothing.
            tracing::debug!(
                "ignoring notification with non-2.0 jsonrpc field: {:?}",
                notif.jsonrpc
            );
        }
        // Known client notifications are accepted-and-discarded:
        //   - notifications/initialized
        //   - notifications/cancelled
        //   - notifications/progress
        // Anything else is silently ignored (spec: "Servers MUST NOT
        // reply to a notification").
    }

    async fn handle_request(&self, req: JsonRpcRequest) -> WireResponse {
        if req.jsonrpc != "2.0" {
            return WireResponse::Err(JsonRpcErrorResponse {
                jsonrpc: "2.0",
                id: req.id,
                error: JsonRpcError::invalid_params(format!(
                    "jsonrpc field must be \"2.0\", got {:?}",
                    req.jsonrpc
                )),
            });
        }

        let result = match req.method.as_str() {
            "initialize" => self.handle_initialize(&req.params),
            "ping" => Ok(json!({})),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(&req.params),
            other => Err(JsonRpcError::method_not_found(other)),
        };

        match result {
            Ok(value) => WireResponse::Ok(JsonRpcResponse {
                jsonrpc: "2.0",
                id: req.id,
                result: value,
            }),
            Err(err) => WireResponse::Err(JsonRpcErrorResponse {
                jsonrpc: "2.0",
                id: req.id,
                error: err,
            }),
        }
    }

    fn handle_initialize(&self, _params: &Value) -> Result<Value, JsonRpcError> {
        // Pinned against tests/fixtures/mcp-spec/server-identity.json
        Ok(json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": MCP_SERVER_NAME,
                "version": MCP_SERVER_VERSION,
            }
        }))
    }

    fn handle_tools_list(&self) -> Result<Value, JsonRpcError> {
        // Prefer the static `T::tool_definitions()` slice (cheap, works
        // for any `ToolProvider`). Fall back to a type-erased
        // `MultiToolProvider` or `DynamicToolRegistry` (T-010) lookup
        // so the runtime registry is honored when the user wraps
        // multiple providers together or uses the dynamic registry.
        let static_tools = T::tool_definitions();
        let tools: Vec<mcp::McpTool> = if !static_tools.is_empty() {
            mcp::to_mcp_tools(static_tools)
        } else {
            crate::server::multi_provider_tool_defs(&*self.provider)
                .or_else(|| crate::server::dynamic_registry_tool_defs(&*self.provider))
                .unwrap_or_default()
        };
        Ok(json!({
            "tools": tools,
        }))
    }

    fn handle_tools_call(&self, params: &Value) -> Result<Value, JsonRpcError> {
        let obj = params
            .as_object()
            .ok_or_else(|| JsonRpcError::invalid_params("params must be an object"))?;
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("params.name must be a string"))?;
        let arguments = obj.get("arguments").cloned().unwrap_or(Value::Null);

        // T-010: when the wrapped provider is a `DynamicToolRegistry`,
        // dispatch through `call_for_tenant` so per-tenant overrides
        // are honored. The stdio transport doesn't have a tenant
        // context yet, so we pass `None` (default-allow); callers
        // wanting per-tenant gating should use the HTTP transport or
        // fork `StdioServer`.
        let result = {
            use std::any::Any;
            if let Some(reg) =
                (&*self.provider as &dyn Any).downcast_ref::<tokitai_core::DynamicToolRegistry>()
            {
                reg.call_for_tenant(name, None, &arguments)
            } else {
                self.provider.call_tool(name, &arguments)
            }
        };
        match result {
            Ok(result) => Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string(&result)
                            .unwrap_or_else(|_| result.to_string()),
                    }
                ],
                "isError": false,
            })),
            Err(e) => Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("{}", e),
                    }
                ],
                "isError": true,
            })),
        }
    }
}

// ============================================================================
// Stdio server builder
// ============================================================================

/// Builder for the stdio MCP transport, mirroring [`McpServerBuilder`].
///
/// Created via [`crate::McpServerBuilder::with_stdio`]; users typically do
/// not construct this directly.
///
/// # Example
///
/// ```rust,ignore
/// use tokitai_mcp_server::McpServerBuilder;
///
/// let builder = McpServerBuilder::with_tool(my_provider).with_stdio();
/// let stdio = builder.build_stdio();
/// stdio.serve_stdio().await?;
/// ```
pub struct McpServerStdioBuilder<T> {
    pub(crate) provider: Arc<T>,
}

impl<T> McpServerStdioBuilder<T>
where
    T: tokitai_core::ToolProvider + tokitai_core::ToolCaller + Send + Sync + 'static,
{
    /// Construct a builder from an already-Arc-wrapped provider. Used by
    /// [`crate::McpServerBuilder::with_stdio`].
    pub(crate) fn from_arc(provider: Arc<T>) -> Self {
        Self { provider }
    }

    /// Build a [`StdioServer`] that drives the wrapped provider over
    /// process stdin/stdout.
    pub fn build(self) -> StdioServer<T> {
        StdioServer {
            provider: self.provider,
        }
    }

    /// Build and immediately run the stdio server, blocking until EOF on
    /// stdin.
    pub async fn serve(self) -> std::io::Result<()> {
        let server = self.build();
        server.serve_stdio().await
    }
}
