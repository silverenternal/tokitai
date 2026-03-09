use tokitai::ToolProvider;

//! Tokitai 入门项目 - 主程序
//!
//! 演示如何：
//! 1. 定义工具
//! 2. 获取工具定义
//! 3. 处理 AI 工具调用请求

mod ai_client;
mod tools;

use serde_json::json;
use tools::{Calculator, WeatherTool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tokitai 入门项目 ===\n");

    // 1. 展示工具定义
    println!("1. 工具定义（可发送给 AI）：");
    println!();

    let weather_tools = WeatherTool::tool_definitions();
    let calculators = Calculator::tool_definitions();

    println!("   天气工具:");
    for tool in weather_tools {
        println!("      - {}: {}", tool.name, tool.description);
    }

    println!();
    println!("   计算器工具:");
    for tool in calculators {
        println!("      - {}: {}", tool.name, tool.description);
    }
    println!();

    // 2. 模拟 AI 调用
    println!("2. 模拟 AI 工具调用：");
    println!();

    let weather = WeatherTool;
    let calculator = Calculator;

    // 模拟天气查询
    println!("   [AI 请求] 查询北京天气");
    let weather_result = weather.call_tool("get_weather", &json!({"city": "北京"}))?;
    println!("   [工具返回] {}", weather_result);
    println!();

    // 模拟数学计算
    println!("   [AI 请求] 计算 100 + 250");
    let add_result = calculator.call_tool("add", &json!({"a": 100, "b": 250}))?;
    println!("   [工具返回] {}", add_result);
    println!();

    println!("   [AI 请求] 计算 100 / 5");
    let div_result = calculator.call_tool("divide", &json!({"dividend": 100, "divisor": 5}))?;
    println!("   [工具返回] {}", div_result);
    println!();

    // 3. 错误处理示例
    println!("3. 错误处理示例：");
    println!();

    println!("   [AI 请求] 计算 100 / 0（除数为零）");
    match calculator.call_tool("divide", &json!({"dividend": 100, "divisor": 0})) {
        Ok(result) => println!("   [工具返回] {}", result),
        Err(e) => println!("   [工具返回错误] {:?}", e),
    }
    println!();

    // 4. 输出 JSON 格式的工具定义（可发送给 AI）
    println!("4. JSON 格式工具定义（示例）：");
    println!();
    println!("   {}", serde_json::to_string_pretty(&weather_tools[0])?);
    println!();

    println!("=== 演示完成 ===");
    println!();
    println!("下一步：");
    println!("  1. 查看 src/tools/ 目录了解如何定义工具");
    println!("  2. 查看 docs/AI_INTEGRATION.md 了解如何集成真实 AI");
    println!("  3. 修改和添加你自己的工具！");

    Ok(())
}
