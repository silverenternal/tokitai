//! Ollama AI integration example
//!
//! Demonstrates how to integrate tokitai with the Ollama API to enable AI tool calls.
//!
//! # Prerequisites
//!
//! 1. Install Ollama: https://ollama.ai
//! 2. Pull a model: `ollama pull llama2` or `ollama pull mistral`
//! 3. Start the Ollama service: `ollama serve`
//!
//! # Configuration
//!
//! Copy `examples/.env.example` to `examples/.env` and configure:
//! ```text
//! # Local Ollama service (default)
//! OLLAMA_BASE_URL=http://localhost:11434
//! OLLAMA_MODEL=llama2
//! OLLAMA_ENABLED=true
//! ```
//!
//! # Run the example
//!
//! ```bash
//! # Run from the project root
//! cargo run --example ollama_integration
//! ```

mod utils;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokitai::tool;
use tokitai::ToolProvider;
use utils::init_console;

// ==================== Environment-Variable Configuration ====================

/// Load configuration from environment variables or defaults
struct Config {
    base_url: String,
    model: String,
    api_key: Option<String>,
    enabled: bool,
}

impl Config {
    fn from_env() -> Self {
        // Try to load examples/.env from the project root
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let env_path = std::path::Path::new(&manifest_dir)
            .join("examples")
            .join(".env");
        dotenv::from_path(env_path).ok();

        // Also try the current-directory .env (compatibility)
        dotenv::dotenv().ok();

        let base_url =
            std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "https://ollama.com".to_string());
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3.5:cloud".to_string());
        let api_key = std::env::var("OLLAMA_API_KEY").ok();
        let enabled = std::env::var("OLLAMA_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";

        Self {
            base_url,
            model,
            api_key,
            enabled,
        }
    }
}

// ==================== Tool Definitions ====================

/// Math calculator tools
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

    /// Compute the square root (rounded)
    pub fn sqrt(&self, n: i32) -> i32 {
        (n as f64).sqrt() as i32
    }
}

/// SHA256 hashing tools
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
pub struct WeatherService;

#[tool]
impl WeatherService {
    /// Get weather information for a specified city
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
            _ => format!("{}: data unavailable", city),
        }
    }
}

/// Time tools
pub struct TimeService;

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

// ==================== Ollama API Type Definitions ====================

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCallResponse>>,
}

#[derive(Debug, Serialize, Clone)]
struct ToolDefinition {
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionDefinition,
}

#[derive(Debug, Serialize, Clone)]
struct FunctionDefinition {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: Message,
    #[allow(dead_code)]
    done: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ToolCallResponse {
    function: FunctionCall,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FunctionCall {
    name: String,
    arguments: Value,
}

// ==================== Ollama Client ====================

struct OllamaClient {
    client: Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl OllamaClient {
    fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            api_key: config.api_key.clone(),
        }
    }

    /// Convert tokitai tool definitions to Ollama format
    #[allow(dead_code)]
    fn convert_tools<T: ToolProvider>(&self) -> Vec<ToolDefinition> {
        T::tool_definitions()
            .iter()
            .map(|tool| {
                let schema: Value = serde_json::from_str(&tool.input_schema).unwrap_or_else(|_| {
                    json!({
                        "type": "object",
                        "properties": {}
                    })
                });

                ToolDefinition {
                    tool_type: "function".to_string(),
                    function: FunctionDefinition {
                        name: tool.name.to_string(),
                        description: tool.description.to_string(),
                        parameters: schema,
                    },
                }
            })
            .collect()
    }

    /// Send a chat request to Ollama
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<Message, String> {
        let request = OllamaRequest {
            model: self.model.clone(),
            messages,
            tools,
            stream: false,
        };

        // The Ollama cloud API path is /api/chat
        let url = format!("{}/api/chat", self.base_url);

        let mut req = self.client.post(&url).json(&request);

        // If an API key is set, add the Authorization header (required by Ollama cloud)
        if let Some(ref api_key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        // Check the response status
        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(format!("API returned error ({}): {}", status, error_text));
        }

        let result = response
            .json::<OllamaResponse>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(result.message)
    }
}

// ==================== Main Flow ====================

struct AiAssistant {
    calculator: Calculator,
    hash_calculator: HashCalculator,
    weather: WeatherService,
    time_service: TimeService,
}

impl AiAssistant {
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

        // Route to the appropriate service based on tool name
        let result = match name {
            "add" | "multiply" | "sqrt" => self
                .calculator
                .call_tool(name, args)
                .map_err(|e| format!("Calculator tool error: {:?}", e))?,
            "sha256" | "sha256_file" => self
                .hash_calculator
                .call_tool(name, args)
                .map_err(|e| format!("Hash tool error: {:?}", e))?,
            "get_weather" => self
                .weather
                .call_tool(name, args)
                .map_err(|e| format!("Weather tool error: {:?}", e))?,
            "get_current_time" | "days_between" => self
                .time_service
                .call_tool(name, args)
                .map_err(|e| format!("Time tool error: {:?}", e))?,
            _ => return Err(format!("Unknown tool: {}", name)),
        };

        Ok(result)
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    init_console();
    println!("=== Tokitai x Ollama AI Integration Example ===\n");

    // Load configuration
    let config = Config::from_env();

    // Check whether Ollama integration is enabled
    if !config.enabled {
        println!("Note: OLLAMA_ENABLED=false, using offline demo mode\n");
        println!("Hint: to enable Ollama integration, follow these steps:");
        println!("  1. Install Ollama: https://ollama.ai");
        println!("  2. Pull a model: ollama pull llama2");
        println!("  3. Start the service: ollama serve");
        println!("  4. Copy examples/.env.example to examples/.env");
        println!("  5. Set OLLAMA_ENABLED=true");
        println!("\nOr use a hosted AI service:");
        println!("  - Edit examples/.env and configure OLLAMA_API_KEY, etc.");
        println!("  - See docs/AI_INTEGRATION.md for integrating other AI services\n");
        run_offline_demo().await.map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Detect cloud vs. local service (API key present or non-localhost address)
    let is_cloud = config.api_key.is_some() && !config.api_key.as_ref().unwrap().is_empty()
        || (!config.base_url.contains("localhost") && !config.base_url.contains("127.0.0.1"));

    if is_cloud {
        println!("Using Ollama cloud service");
        println!("  Service URL: {}", config.base_url);
        println!("  Model: {}", config.model);
        if config.api_key.is_some() && !config.api_key.as_ref().unwrap().is_empty() {
            println!("  API Key: configured\n");
        } else {
            println!("  API Key: not configured (may be required)\n");
        }
    } else {
        // Local-service check
        println!("Checking Ollama local service at {}...", config.base_url);
        let client = reqwest::Client::new();
        match client
            .get(format!("{}/api/tags", config.base_url))
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    println!("Ollama local service is up\n");
                } else {
                    println!("Ollama local service returned an abnormal response, continuing with the demo...\n");
                }
            }
            Err(_) => {
                println!(
                    "Ollama local service is not running, falling back to offline demo mode\n"
                );
                run_offline_demo().await.map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
    }

    // Create the assistant and Ollama client
    let assistant = AiAssistant::new();
    let ollama = OllamaClient::new(&config);

    // Collect all tool definitions
    let mut all_tools = Vec::new();
    all_tools.extend(Calculator::tool_definitions().iter().map(convert_tool_def));
    all_tools.extend(
        HashCalculator::tool_definitions()
            .iter()
            .map(convert_tool_def),
    );
    all_tools.extend(
        WeatherService::tool_definitions()
            .iter()
            .map(convert_tool_def),
    );
    all_tools.extend(TimeService::tool_definitions().iter().map(convert_tool_def));

    println!("Loaded {} tools:", all_tools.len());
    for tool in &all_tools {
        println!("   - {}: {}", tool.function.name, tool.function.description);
    }
    println!();

    // Simulated conversation - SHA256 test (the AI can't compute the hash precisely)
    let mut messages = vec![Message {
        role: "user".to_string(),
        content: "Please compute the SHA256 hash of the string 'hello world'".to_string(),
        tool_calls: None,
    }];

    println!("[User] Please compute the SHA256 hash of the string 'hello world'\n");

    // First turn
    let response = ollama
        .chat(messages.clone(), Some(all_tools.clone()))
        .await?;

    if let Some(tool_calls) = response.tool_calls {
        println!("[AI requests tool call]");
        for call in tool_calls {
            let result =
                assistant.handle_tool_call(&call.function.name, &call.function.arguments)?;
            println!("   [Tool return] {}\n", result);

            // Append the tool call and its result to the message history
            messages.push(Message {
                role: "assistant".to_string(),
                content: "".to_string(),
                tool_calls: Some(vec![call]),
            });
            messages.push(Message {
                role: "tool".to_string(),
                content: result.to_string(),
                tool_calls: None,
            });
        }

        // Get the final reply
        let final_response = ollama
            .chat(messages, None)
            .await
            .map_err(|e| e.to_string())?;
        println!("[AI final reply] {}", final_response.content);
    } else {
        println!("[AI reply] {}", response.content);
    }

    Ok(())
}

/// Offline demo mode (used when Ollama is unavailable)
async fn run_offline_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Offline Demo Mode ===\n");

    let assistant = AiAssistant::new();

    // Show tool definitions
    println!("1. Tool definitions (can be sent to any AI)");
    let tools = Calculator::tool_definitions();
    for tool in tools {
        println!(
            "   {{ \"name\": \"{}\", \"description\": \"{}\", \"input_schema\": {} }}",
            tool.name, tool.description, tool.input_schema
        );
    }

    let hash_tools = HashCalculator::tool_definitions();
    for tool in hash_tools {
        println!(
            "   {{ \"name\": \"{}\", \"description\": \"{}\", \"input_schema\": {} }}",
            tool.name, tool.description, tool.input_schema
        );
    }
    println!();

    // Simulated AI conversation flow
    println!("2. Simulated AI conversation flow");
    println!("   [User] Please compute 100 + 250");
    println!("   [AI] Let me compute that...");

    let result = assistant.handle_tool_call("add", &json!({"a": 100, "b": 250}))?;
    println!("   [Tool execution] {}", result);
    println!("   [AI] The result is {}\n", result);

    // SHA256 example (the AI cannot compute it precisely)
    println!("3. SHA256 hashing (the AI cannot compute it precisely)");
    println!("   [User] Compute the SHA256 of 'hello world'");
    println!("   [AI] Let me use a tool to compute it...");

    let result = assistant.handle_tool_call("sha256", &json!({"input": "hello world"}))?;
    println!("   [Tool execution] {}", result);
    println!("   [AI] The SHA256 hash is: {}\n", result);

    println!("4. Weather lookup example");
    println!("   [User] What's the weather in Beijing?");
    let result = assistant.handle_tool_call("get_weather", &json!({"city": "Beijing"}))?;
    println!("   [Tool execution] {}", result);
    println!("   [AI] {}\n", result);

    println!("Hint: install and start Ollama to experience the full AI integration:");
    println!("   1. Visit https://ollama.ai to download and install");
    println!("   2. Run: ollama pull llama2");
    println!("   3. Run: ollama serve");
    println!("   4. Re-run this example");

    Ok(())
}

fn convert_tool_def(tool: &tokitai::ToolDefinition) -> ToolDefinition {
    let schema: Value = serde_json::from_str(&tool.input_schema).unwrap_or_else(|_| {
        json!({
            "type": "object",
            "properties": {}
        })
    });

    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: tool.name.to_string(),
            description: tool.description.to_string(),
            parameters: schema,
        },
    }
}
