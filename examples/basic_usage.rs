//! 基础使用示例
//!
//! 展示如何使用 #[tool] 宏快速将 Rust 方法暴露给 AI 调用

use tokitai::tool;

/// 简单计算器
pub struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 两个数相乘
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }

    /// 从当前值减去一个数
    pub fn subtract(&self, a: i32, b: i32) -> i32 {
        a - b
    }

    /// 除法运算
    pub fn divide(&self, dividend: i32, divisor: i32) -> Result<i32, String> {
        if divisor == 0 {
            Err("除数不能为零".to_string())
        } else {
            Ok(dividend / divisor)
        }
    }
}

/// 自定义工具属性示例
pub struct WeatherService;

#[tool]
impl WeatherService {
    /// 获取指定城市的天气信息
    #[tool(name = "get_weather", desc = "获取城市天气预报")]
    pub fn get_weather(&self, city: String) -> String {
        format!("{} 的天气：晴朗，温度 25°C", city)
    }

    /// 获取多日天气预报
    #[tool(name = "get_forecast", desc = "获取多日天气预报")]
    pub fn get_forecast(&self, _city: String, days: i32) -> Vec<String> {
        (0..days)
            .map(|i| format!("第 {} 天：晴朗", i + 1))
            .collect()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== Tokitai 基础使用示例 ===\n");

    // 示例 1: 获取工具定义
    println!("1. 获取工具定义（发送给 AI）");
    let tools = Calculator::TOOL_DEFINITIONS;
    println!("   工具数量：{}", tools.len());
    for tool in tools {
        println!("   - {}: {}", tool.name, tool.description);
    }
    println!();

    // 示例 2: 调用工具
    println!("2. 调用工具（模拟 AI 请求）");
    let calc = Calculator;

    // 模拟 AI 返回的工具调用
    let result = calc
        .call_tool("add", &serde_json::json!({"a": 10, "b": 20}))?;
    println!("   add(10, 20) = {}", result);

    let result = calc
        .call_tool("multiply", &serde_json::json!({"a": 5, "b": 6}))?;
    println!("   multiply(5, 6) = {}", result);
    println!();

    // 示例 3: 错误处理
    println!("3. 错误处理");
    match calc
        .call_tool("divide", &serde_json::json!({"dividend": 10, "divisor": 0}))
    {
        Ok(_) => println!("   不应该成功"),
        Err(e) => println!("   捕获错误：{:?}", e),
    }
    println!();

    // 示例 4: 自定义工具属性
    println!("4. 自定义工具属性");
    let weather = WeatherService;
    let weather_tools = WeatherService::TOOL_DEFINITIONS;
    for tool in weather_tools {
        println!("   - {} (自定义名称): {}", tool.name, tool.description);
    }

    let result = weather
        .call_tool("get_weather", &serde_json::json!({"city": "北京"}))?;
    println!("   get_weather(北京) = {}", result);
    println!();

    // 示例 5: 模拟完整的 AI 对话流程
    println!("5. 完整 AI 对话流程模拟");
    simulate_ai_conversation()?;

    Ok(())
}

/// 模拟完整的 AI 对话流程
fn simulate_ai_conversation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let calc = Calculator;

    // 步骤 1: 发送工具定义给 AI
    let tools = Calculator::TOOL_DEFINITIONS;
    println!("   [发送给 AI] 工具定义：{} 个", tools.len());

    // 步骤 2: 模拟 AI 返回的工具调用
    let ai_tool_call = serde_json::json!({
        "tool_name": "add",
        "arguments": {
            "a": 100,
            "b": 200
        }
    });

    println!(
        "   [AI 调用] {}({:?})",
        ai_tool_call["tool_name"], ai_tool_call["arguments"]
    );

    // 步骤 3: 执行工具调用
    let result = calc
        .call_tool(
            ai_tool_call["tool_name"].as_str().unwrap(),
            &ai_tool_call["arguments"],
        )
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    println!("   [执行结果] {}", result);

    // 步骤 4: 返回结果给 AI
    println!("   [返回给 AI] 结果：{}", result);

    Ok(())
}
