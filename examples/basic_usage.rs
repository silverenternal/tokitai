//! 5-minute quick start
//!
//! ```bash
//! cargo run --example basic_usage
//! ```

mod utils;
use tokitai::tool;
use tokitai::ToolProvider;
use utils::init_console;

/// Simple Calculator
pub struct Calculator;

#[tool]
impl Calculator {
    /// Adds two numbers together
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Multiplies two numbers
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }

    /// Divides two numbers
    pub fn divide(&self, dividend: i32, divisor: i32) -> Result<i32, String> {
        if divisor == 0 {
            Err("divisor cannot be zero".to_string())
        } else {
            Ok(dividend / divisor)
        }
    }
}

/// Custom Tool Attributes Example
pub struct WeatherService;

#[tool]
impl WeatherService {
    /// Get weather information for a specified city
    #[tool(
        name = "get_weather",
        desc = "Returns the current weather forecast for the given city as a human-readable String. Requires the city name to be a non-empty string."
    )]
    pub fn get_weather(&self, city: String) -> String {
        format!("Weather in {}: clear, temperature 25C", city)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_console();
    println!("=== Tokitai Basic Usage Example ===\n");

    // 1. Get tool definitions (send to AI)
    println!("1. Get Tool Definitions (send to AI)");
    let tools = Calculator::tool_definitions();
    println!("   Tool count: {}", tools.len());
    for tool in tools {
        println!("   - {}: {}", tool.name, tool.description);
    }
    println!();

    // 2. Call tools (simulate AI request)
    println!("2. Call Tools (simulate AI request)");
    let calc = Calculator;

    let result = calc.call_tool("add", &tokitai::json!({"a": 10, "b": 20}))?;
    println!("   add(10, 20) = {}", result);

    let result = calc.call_tool("multiply", &tokitai::json!({"a": 5, "b": 6}))?;
    println!("   multiply(5, 6) = {}", result);
    println!();

    // 3. Error handling
    println!("3. Error Handling");
    match calc.call_tool("divide", &tokitai::json!({"dividend": 10, "divisor": 0})) {
        Ok(_) => println!("   Should not succeed"),
        Err(e) => println!("   Caught error: {:?}", e),
    }
    println!();

    // 4. Custom tool attributes
    println!("4. Custom Tool Attributes");
    let weather = WeatherService;
    let weather_tools = WeatherService::tool_definitions();
    for tool in weather_tools {
        println!("   - {} (custom name): {}", tool.name, tool.description);
    }

    let result = weather.call_tool("get_weather", &tokitai::json!({"city": "Beijing"}))?;
    println!("   get_weather(Beijing) = {}", result);
    println!();

    // 5. Simulate a complete AI conversation flow
    println!("5. Complete AI Conversation Flow");
    simulate_ai_conversation()?;

    Ok(())
}

/// Simulate a complete AI conversation flow
fn simulate_ai_conversation() -> Result<(), Box<dyn std::error::Error>> {
    let calc = Calculator;

    // Step 1: send tool definitions to the AI
    let tools = Calculator::tool_definitions();
    println!("   [Send to AI] Tool definitions: {} tools", tools.len());

    // Step 2: simulate the tool call returned by the AI
    let ai_tool_call = tokitai::json!({
        "tool_name": "add",
        "arguments": {
            "a": 100,
            "b": 200
        }
    });

    println!(
        "   [AI Call] {}({:?})",
        ai_tool_call["tool_name"], ai_tool_call["arguments"]
    );

    // Step 3: execute the tool call
    let result = calc.call_tool(
        ai_tool_call["tool_name"].as_str().unwrap(),
        &ai_tool_call["arguments"],
    )?;

    println!("   [Execute Result] {}", result);

    // Step 4: return the result to the AI
    println!("   [Return to AI] Result: {}", result);

    Ok(())
}
