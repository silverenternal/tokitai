//! 5 分钟快速开始
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
            Err("除数不能为零".to_string())
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
    #[tool(name = "get_weather", desc = "获取城市天气预报")]
    pub fn get_weather(&self, city: String) -> String {
        format!("{} 的天气：晴朗，温度 25°C", city)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_console();
    println!("=== Tokitai Basic Usage Example ===\n");

    // 1. 获取工具定义（发送给 AI）
    println!("1. Get Tool Definitions (send to AI)");
    let tools = Calculator::tool_definitions();
    println!("   Tool count: {}", tools.len());
    for tool in tools {
        println!("   - {}: {}", tool.name, tool.description);
    }
    println!();

    // 2. 调用工具（模拟 AI 请求）
    println!("2. Call Tools (simulate AI request)");
    let calc = Calculator;

    let result = calc.call_tool("add", &tokitai::json!({"a": 10, "b": 20}))?;
    println!("   add(10, 20) = {}", result);

    let result = calc.call_tool("multiply", &tokitai::json!({"a": 5, "b": 6}))?;
    println!("   multiply(5, 6) = {}", result);
    println!();

    // 3. 错误处理
    println!("3. Error Handling");
    match calc.call_tool("divide", &tokitai::json!({"dividend": 10, "divisor": 0})) {
        Ok(_) => println!("   Should not succeed"),
        Err(e) => println!("   Caught error: {:?}", e),
    }
    println!();

    // 4. 自定义工具属性
    println!("4. Custom Tool Attributes");
    let weather = WeatherService;
    let weather_tools = WeatherService::tool_definitions();
    for tool in weather_tools {
        println!("   - {} (custom name): {}", tool.name, tool.description);
    }

    let result = weather.call_tool("get_weather", &tokitai::json!({"city": "北京"}))?;
    println!("   get_weather(北京) = {}", result);
    println!();

    // 5. 模拟完整的 AI 对话流程
    println!("5. Complete AI Conversation Flow");
    simulate_ai_conversation()?;

    Ok(())
}

/// 模拟完整的 AI 对话流程
fn simulate_ai_conversation() -> Result<(), Box<dyn std::error::Error>> {
    let calc = Calculator;

    // 步骤 1: 发送工具定义给 AI
    let tools = Calculator::tool_definitions();
    println!("   [Send to AI] Tool definitions: {} tools", tools.len());

    // 步骤 2: 模拟 AI 返回的工具调用
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

    // 步骤 3: 执行工具调用
    let result = calc.call_tool(
        ai_tool_call["tool_name"].as_str().unwrap(),
        &ai_tool_call["arguments"],
    )?;

    println!("   [Execute Result] {}", result);

    // 步骤 4: 返回结果给 AI
    println!("   [Return to AI] Result: {}", result);

    Ok(())
}
