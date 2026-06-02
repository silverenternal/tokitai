//! Complete MCP Server example
//!
//! Demonstrates how to build a complete MCP server that supports AI tool calls.
//!
//! # How to run
//!
//! ```bash
//! # Run from the project root
//! cargo run --example mcp_server_demo
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

mod utils;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokitai::tool;
use tokitai::ToolProvider;
use utils::init_console;

// ==================== Tool Definitions ====================

/// Math calculator tools
#[derive(Default, Clone)]
pub struct Calculator;

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

    /// Compute the square root (returns the integer part)
    pub fn sqrt(&self, n: i32) -> i32 {
        (n as f64).sqrt() as i32
    }

    /// Compute power (base^exp)
    pub fn power(&self, base: i32, exp: u32) -> i32 {
        base.pow(exp)
    }
}

/// SHA256 hashing tools
#[derive(Default, Clone)]
pub struct HashCalculator;

#[tool]
impl HashCalculator {
    /// Compute the SHA256 hash of a string (returns a 64-character hex string)
    pub fn sha256(&self, input: String) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Compute the SHA256 hash of a file
    pub fn sha256_file(&self, path: String) -> String {
        match std::fs::read(&path) {
            Ok(contents) => {
                let mut hasher = Sha256::new();
                hasher.update(&contents);
                let result = hasher.finalize();
                hex::encode(result)
            }
            Err(e) => format!("Failed to read file: {}", e),
        }
    }
}

/// Weather lookup tools
#[derive(Default, Clone)]
pub struct WeatherService;

#[tool]
impl WeatherService {
    /// Get weather information for a specified city
    #[tool(tags = ["weather", "query"])]
    pub fn get_weather(&self, city: String) -> String {
        // Simulated weather data
        match city.to_lowercase().as_str() {
            "beijing" => "Beijing: clear, temperature 25C, humidity 40%".to_string(),
            "shanghai" => "Shanghai: cloudy, temperature 22C, humidity 60%".to_string(),
            "guangzhou" => "Guangzhou: light rain, temperature 28C, humidity 80%".to_string(),
            "shenzhen" => "Shenzhen: clear, temperature 30C, humidity 70%".to_string(),
            "hangzhou" => "Hangzhou: cloudy, temperature 24C, humidity 65%".to_string(),
            "chengdu" => "Chengdu: overcast, temperature 20C, humidity 75%".to_string(),
            "chongqing" => "Chongqing: light rain, temperature 26C, humidity 85%".to_string(),
            "wuhan" => "Wuhan: clear, temperature 27C, humidity 55%".to_string(),
            "xian" => "Xian: cloudy, temperature 23C, humidity 50%".to_string(),
            "nanjing" => "Nanjing: clear, temperature 25C, humidity 60%".to_string(),
            "new york" => "New York: clear, temperature 20C, humidity 45%".to_string(),
            "london" => "London: overcast, temperature 15C, humidity 70%".to_string(),
            "tokyo" => "Tokyo: cloudy, temperature 22C, humidity 60%".to_string(),
            _ => format!("{}: data unavailable", city),
        }
    }

    /// Compare weather across multiple cities
    pub fn compare_weather(&self, cities: Vec<String>) -> String {
        let results: Vec<String> = cities
            .iter()
            .map(|city| format!("{}: {}", city, self.get_weather(city.clone())))
            .collect();
        results.join("\n")
    }
}

/// Time tools
#[derive(Default, Clone)]
pub struct TimeService;

#[tool]
impl TimeService {
    /// Get the current date and time (format: YYYY-MM-DD HH:MM:SS)
    pub fn get_current_time(&self) -> String {
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// Get the current date (format: YYYY-MM-DD)
    pub fn get_current_date(&self) -> String {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    }

    /// Compute the number of days between two dates
    pub fn days_between(&self, date1: String, date2: String) -> Result<i32, String> {
        let d1 = chrono::NaiveDate::parse_from_str(&date1, "%Y-%m-%d")
            .map_err(|e| format!("Invalid date format: {}", e))?;
        let d2 = chrono::NaiveDate::parse_from_str(&date2, "%Y-%m-%d")
            .map_err(|e| format!("Invalid date format: {}", e))?;
        Ok((d2 - d1).num_days() as i32)
    }

    /// Format a date
    #[tool(example_format = "%Y/%m/%d")]
    pub fn format_date(&self, date: String, format: Option<String>) -> Result<String, String> {
        let d = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map_err(|e| format!("Invalid date format: {}", e))?;
        let fmt = format.as_deref().unwrap_or("%Y-%m-%d");
        Ok(d.format(fmt).to_string())
    }
}

// ==================== Tool-call Handler ====================

/// Unified tool-call handler
struct ToolHandler {
    calculator: Calculator,
    hash_calculator: HashCalculator,
    weather: WeatherService,
    time_service: TimeService,
}

impl ToolHandler {
    fn new() -> Self {
        Self {
            calculator: Calculator,
            hash_calculator: HashCalculator,
            weather: WeatherService,
            time_service: TimeService,
        }
    }

    /// Handle a tool call
    fn handle_tool_call(&self, name: &str, args: &Value) -> Result<Value, String> {
        println!("   [Tool call] {}({:?})", name, args);

        let result = match name {
            // Calculator
            "add" | "multiply" | "sqrt" | "power" => self
                .calculator
                .call_tool(name, args)
                .map_err(|e| format!("Calculator tool error: {:?}", e))?,
            // HashCalculator
            "sha256" | "sha256_file" => self
                .hash_calculator
                .call_tool(name, args)
                .map_err(|e| format!("Hash tool error: {:?}", e))?,
            // WeatherService
            "get_weather" | "compare_weather" => self
                .weather
                .call_tool(name, args)
                .map_err(|e| format!("Weather tool error: {:?}", e))?,
            // TimeService
            "get_current_time" | "get_current_date" | "days_between" | "format_date" => self
                .time_service
                .call_tool(name, args)
                .map_err(|e| format!("Time tool error: {:?}", e))?,
            _ => return Err(format!("Unknown tool: {}", name)),
        };

        Ok(result)
    }

    /// Get all tool definitions
    fn get_all_tools(&self) -> Vec<tokitai::ToolDefinition> {
        let mut tools = Vec::new();
        tools.extend(Calculator::tool_definitions().iter().cloned());
        tools.extend(HashCalculator::tool_definitions().iter().cloned());
        tools.extend(WeatherService::tool_definitions().iter().cloned());
        tools.extend(TimeService::tool_definitions().iter().cloned());
        tools
    }
}

// ==================== Main Flow ====================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_console();

    println!("=== Tokitai MCP Server Example ===\n");

    // Create the tool handler
    let handler = Arc::new(ToolHandler::new());

    // Show tool definitions
    println!("Loaded tools:");
    for tool in handler.get_all_tools() {
        println!("  - {}: {}", tool.name, tool.description);
    }
    println!();

    // Convert to MCP format
    let mcp_tools = tokitai::mcp::to_mcp_tools(&handler.get_all_tools());
    println!("MCP tool definitions ({}):", mcp_tools.len());
    for tool in &mcp_tools {
        println!(
            "  - {} ({} bytes)",
            tool.name,
            tool.input_schema.to_string().len()
        );
    }
    println!();

    // Demonstrate tool calls
    println!("=== Tool-call Demonstrations ===\n");

    // 1. Math
    println!("[1] Math operations");
    let result = handler.handle_tool_call("add", &json!({"a": 100, "b": 250}))?;
    println!("    add(100, 250) = {}\n", result);

    let result = handler.handle_tool_call("multiply", &json!({"a": 12, "b": 8}))?;
    println!("    multiply(12, 8) = {}\n", result);

    // 2. SHA256
    println!("[2] SHA256 hashing");
    let result = handler.handle_tool_call("sha256", &json!({"input": "hello world"}))?;
    println!("    sha256('hello world') = {}\n", result);

    // 3. Weather
    println!("[3] Weather lookup");
    let result = handler.handle_tool_call("get_weather", &json!({"city": "Beijing"}))?;
    println!("    get_weather('Beijing') = {}\n", result);

    // 4. Time
    println!("[4] Time lookup");
    let result = handler.handle_tool_call("get_current_time", &json!({}))?;
    println!("    get_current_time() = {}\n", result);

    // Hint for starting the HTTP server
    println!("=== Start the HTTP Server ===\n");
    println!("Hint: to start the full HTTP MCP server, run:");
    println!("  cargo run --example mcp_http_server\n");
    println!("Server endpoints:");
    println!("  GET  http://127.0.0.1:8080/tools  - list tools");
    println!("  POST http://127.0.0.1:8080/call   - call a tool");
    println!("  GET  http://127.0.0.1:8080/health - health check\n");

    // Show how to integrate with AI
    println!("=== AI Integration Example ===\n");
    println!("Python example:");
    println!(
        r#"
import requests

# List tools
response = requests.get("http://127.0.0.1:8080/tools")
tools = response.json()

# Call a tool
response = requests.post(
    "http://127.0.0.1:8080/call",
    json={{"name": "add", "arguments": {{"a": 10, "b": 20}}}}
)
result = response.json()
print(f"Result: {{result['result']}}")  # 30
"#
    );

    Ok(())
}
