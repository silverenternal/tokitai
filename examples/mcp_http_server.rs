//! MCP HTTP server example
//!
//! Demonstrates how to run a complete MCP HTTP server.
//!
//! # How to run
//!
//! ```bash
//! cargo run --example mcp_http_server
//! ```
//!
//! # Test the API
//!
//! ```bash
//! # List tools
//! curl http://127.0.0.1:8080/tools
//!
//! # Call a tool
//! curl -X POST http://127.0.0.1:8080/call \
//!   -H "Content-Type: application/json" \
//!   -d '{"name": "add", "arguments": {"a": 10, "b": 20}}'
//!
//! # Health check
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

// ==================== Tool Definitions ====================

#[derive(Default, Clone)]
struct Calculator;

#[tool]
impl Calculator {
    /// Add two numbers
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Multiply two numbers
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }

    /// Compute the square root
    pub fn sqrt(&self, n: i32) -> i32 {
        (n as f64).sqrt() as i32
    }
}

#[derive(Default, Clone)]
struct HashCalculator;

#[tool]
impl HashCalculator {
    /// Compute the SHA256 hash of a string
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
    /// Get weather information for a specified city
    pub fn get_weather(&self, city: String) -> String {
        match city.to_lowercase().as_str() {
            "beijing" => "Beijing: clear, temperature 25C, humidity 40%".to_string(),
            "shanghai" => "Shanghai: cloudy, temperature 22C, humidity 60%".to_string(),
            "guangzhou" => "Guangzhou: light rain, temperature 28C, humidity 80%".to_string(),
            "shenzhen" => "Shenzhen: clear, temperature 30C, humidity 70%".to_string(),
            "new york" => "New York: clear, temperature 20C, humidity 45%".to_string(),
            "london" => "London: overcast, temperature 15C, humidity 70%".to_string(),
            "tokyo" => "Tokyo: cloudy, temperature 22C, humidity 60%".to_string(),
            _ => format!("{}: data unavailable", city),
        }
    }
}

#[derive(Default, Clone)]
struct TimeService;

#[tool]
impl TimeService {
    /// Get the current date and time
    pub fn get_current_time(&self) -> String {
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// Compute the number of days between two dates
    pub fn days_between(&self, date1: String, date2: String) -> Result<i32, String> {
        let d1 = chrono::NaiveDate::parse_from_str(&date1, "%Y-%m-%d")
            .map_err(|e| format!("Invalid date format: {}", e))?;
        let d2 = chrono::NaiveDate::parse_from_str(&date2, "%Y-%m-%d")
            .map_err(|e| format!("Invalid date format: {}", e))?;
        Ok((d2 - d1).num_days() as i32)
    }
}

// ==================== Application State ====================

/// Application state
struct AppState {
    tools: Vec<tokitai::mcp::McpTool>,
}

/// Tool-call request
#[derive(Debug, Deserialize)]
struct ToolCallRequest {
    name: String,
    #[serde(default)]
    arguments: Value,
}

/// Tool-call response
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

// ==================== HTTP Handlers ====================

/// List-tools handler
async fn list_tools_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<tokitai::mcp::McpTool>> {
    info!("Listing {} tools", state.tools.len());
    Json(state.tools.clone())
}

/// Call-tool handler
async fn call_tool_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ToolCallRequest>,
) -> Result<Json<ToolCallResponse>, StatusCode> {
    info!(
        "Tool call request: name={}, arguments={:?}",
        request.name, request.arguments
    );

    // Find the tool
    let tool = state
        .tools
        .iter()
        .find(|t| t.name == request.name)
        .ok_or_else(|| {
            warn!("Tool not found: {}", request.name);
            StatusCode::NOT_FOUND
        })?;

    info!("Found tool: {} - {}", tool.name, tool.description);

    // Actually invoke the tool
    // Note: in real applications you would maintain a tool-instance map
    let result = call_concrete_tool(&request.name, &request.arguments);

    match result {
        Ok(value) => Ok(Json(ToolCallResponse::success(value))),
        Err(e) => {
            error!("Tool execution error: {}", e);
            Ok(Json(ToolCallResponse::error(e)))
        }
    }
}

/// Health-check handler
async fn health_handler() -> &'static str {
    "OK"
}

// ==================== Tool-call Routing ====================

/// Helper that dispatches to a concrete tool instance
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

// ==================== Main Flow ====================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tokitai=info,axum=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("=== Tokitai MCP HTTP Server ===\n");

    // Collect all tool definitions
    let mut all_tools = Vec::new();
    all_tools.extend(tokitai::mcp::to_mcp_tools(Calculator::tool_definitions()).into_iter());
    all_tools.extend(tokitai::mcp::to_mcp_tools(HashCalculator::tool_definitions()).into_iter());
    all_tools.extend(tokitai::mcp::to_mcp_tools(WeatherService::tool_definitions()).into_iter());
    all_tools.extend(tokitai::mcp::to_mcp_tools(TimeService::tool_definitions()).into_iter());

    println!("Loaded {} tools:", all_tools.len());
    for tool in &all_tools {
        println!("  - {}: {}", tool.name, tool.description);
    }
    println!();

    // Create application state
    let state = Arc::new(AppState { tools: all_tools });

    // Build the router
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

    // Start the server
    let addr = "127.0.0.1:8080";
    println!("Starting MCP HTTP server...");
    println!("Listening on http://{}", addr);
    println!();
    println!("API endpoints:");
    println!("  GET  /tools  - list tools");
    println!("  POST /call   - call a tool");
    println!("  GET  /health - health check");
    println!();
    println!("Test commands:");
    println!("  curl http://127.0.0.1:8080/tools");
    println!("  curl -X POST http://127.0.0.1:8080/call -H \"Content-Type: application/json\" -d '{{\"name\":\"add\",\"arguments\":{{\"a\":10,\"b\":20}}}}'");
    println!();
    println!("Press Ctrl+C to stop the server\n");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
