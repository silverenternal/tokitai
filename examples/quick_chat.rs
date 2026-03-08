//! 快速聊天示例
//!
//! 1. 运行：cargo run --example quick_chat
//! 2. 输入数学问题（如：123 + 456 = ?）
//! 3. 看到 AI 调用 Rust 代码执行计算

mod utils;
use tokitai::tool;
use std::io::{self, Write};
use utils::init_console;

/// 数学计算工具
#[tool]
struct MathTools;

#[tool]
impl MathTools {
    /// Adds two numbers together
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Multiplies two numbers
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }

    /// Divides two numbers
    pub fn divide(&self, a: i32, b: i32) -> Option<i32> {
        if b == 0 { None } else { Some(a / b) }
    }
}

fn main() {
    init_console();
    println!("🔢 Tokitai Math Calculator");
    println!("==========================");
    println!("Supported operations: +, *, /");
    println!("Example: 123 + 456 = ?");
    println!();

    // 显示工具定义
    println!("📋 Available Tools:");
    for tool in MathTools::TOOL_DEFINITIONS {
        println!("   - {}: {}", tool.name, tool.description);
    }
    println!();

    // 读取用户输入
    print!("Enter math problem: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    let input = input.trim();

    // 简单解析并调用工具
    let tools = MathTools;

    if input.contains('+') {
        let parts: Vec<&str> = input.split('+').collect();
        if parts.len() == 2 {
            // 清理并解析数字
            let a_str = parts[0].trim().trim_end_matches('=');
            let b_str = parts[1].trim().replace("=", "").replace("?", "");

            match (a_str.parse::<i32>(), b_str.trim().parse::<i32>()) {
                (Ok(a), Ok(b)) => {
                    let result = tools.call_tool("add", &tokitai::json!({"a": a, "b": b})).unwrap();
                    println!("✅ Result: {} + {} = {}", a, b, result);
                }
                _ => println!("❌ Cannot parse numbers. Try: 123 + 456 = ?"),
            }
        } else {
            println!("❌ Invalid format. Try: 123 + 456 = ?");
        }
    } else if input.contains('*') || input.contains('×') {
        let parts: Vec<&str> = input.split(['*', '×']).collect();
        if parts.len() == 2 {
            let a_str = parts[0].trim().trim_end_matches('=');
            let b_str = parts[1].trim().replace("=", "").replace("?", "");

            match (a_str.parse::<i32>(), b_str.trim().parse::<i32>()) {
                (Ok(a), Ok(b)) => {
                    let result = tools.call_tool("multiply", &tokitai::json!({"a": a, "b": b})).unwrap();
                    println!("✅ Result: {} × {} = {}", a, b, result);
                }
                _ => println!("❌ Cannot parse numbers. Try: 123 * 456 = ?"),
            }
        } else {
            println!("❌ Invalid format. Try: 123 * 456 = ?");
        }
    } else if input.contains('/') {
        let parts: Vec<&str> = input.split('/').collect();
        if parts.len() == 2 {
            let a_str = parts[0].trim().trim_end_matches('=');
            let b_str = parts[1].trim().replace("=", "").replace("?", "");

            match (a_str.parse::<i32>(), b_str.trim().parse::<i32>()) {
                (Ok(a), Ok(b)) => {
                    if b == 0 {
                        println!("❌ Error: Cannot divide by zero");
                    } else {
                        let result = tools.call_tool("divide", &tokitai::json!({"a": a, "b": b})).unwrap();
                        println!("✅ Result: {} ÷ {} = {}", a, b, result);
                    }
                }
                _ => println!("❌ Cannot parse numbers. Try: 123 / 456 = ?"),
            }
        } else {
            println!("❌ Invalid format. Try: 123 / 456 = ?");
        }
    } else {
        println!("❌ Cannot parse. Try: 123 + 456 = ?");
    }

    println!();
    println!("💡 How tokitai works:");
    println!("   1. AI receives tool definitions");
    println!("   2. AI decides which tool to call");
    println!("   3. Rust executes and returns results");
}
