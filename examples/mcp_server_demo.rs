//! MCP Server 完整示例
//!
//! 展示如何构建一个完整的 MCP 服务器，支持 AI 调用工具。
//!
//! # 运行方式
//!
//! ```bash
//! # 从项目根目录运行
//! cargo run --example mcp_server_demo
//! ```
//!
//! # 测试 API
//!
//! ```bash
//! # 获取工具列表
//! curl http://127.0.0.1:8080/tools
//!
//! # 调用工具
//! curl -X POST http://127.0.0.1:8080/call \
//!   -H "Content-Type: application/json" \
//!   -d '{"name": "add", "arguments": {"a": 10, "b": 20}}'
//!
//! # 健康检查
//! curl http://127.0.0.1:8080/health
//! ```

mod utils;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokitai::tool;
use tokitai::ToolProvider;
use utils::init_console;

// ==================== 工具定义 ====================

/// 数学计算器工具
#[derive(Default, Clone)]
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

    /// 计算平方根（返回整数部分）
    pub fn sqrt(&self, n: i32) -> i32 {
        (n as f64).sqrt() as i32
    }

    /// 计算幂运算 (base^exp)
    pub fn power(&self, base: i32, exp: u32) -> i32 {
        base.pow(exp)
    }
}

/// SHA256 哈希计算工具
#[derive(Default, Clone)]
pub struct HashCalculator;

#[tool]
impl HashCalculator {
    /// 计算字符串的 SHA256 哈希值（返回 64 位十六进制）
    pub fn sha256(&self, input: String) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// 计算文件的 SHA256 哈希值
    pub fn sha256_file(&self, path: String) -> String {
        match std::fs::read(&path) {
            Ok(contents) => {
                let mut hasher = Sha256::new();
                hasher.update(&contents);
                let result = hasher.finalize();
                hex::encode(result)
            }
            Err(e) => format!("读取文件失败：{}", e),
        }
    }
}

/// 天气查询工具
#[derive(Default, Clone)]
pub struct WeatherService;

#[tool]
impl WeatherService {
    /// 获取指定城市的天气信息
    #[tool(tags = ["weather", "query"])]
    pub fn get_weather(&self, city: String) -> String {
        // 模拟天气数据
        match city.to_lowercase().as_str() {
            "北京" | "beijing" => "北京：晴朗，温度 25°C，湿度 40%".to_string(),
            "上海" | "shanghai" => "上海：多云，温度 22°C，湿度 60%".to_string(),
            "广州" | "guangzhou" => "广州：小雨，温度 28°C，湿度 80%".to_string(),
            "深圳" | "shenzhen" => "深圳：晴朗，温度 30°C，湿度 70%".to_string(),
            "杭州" | "hangzhou" => "杭州：多云，温度 24°C，湿度 65%".to_string(),
            "成都" | "chengdu" => "成都：阴天，温度 20°C，湿度 75%".to_string(),
            "重庆" | "chongqing" => "重庆：小雨，温度 26°C，湿度 85%".to_string(),
            "武汉" | "wuhan" => "武汉：晴朗，温度 27°C，湿度 55%".to_string(),
            "西安" | "xian" => "西安：多云，温度 23°C，湿度 50%".to_string(),
            "南京" | "nanjing" => "南京：晴朗，温度 25°C，湿度 60%".to_string(),
            "纽约" | "new york" => "纽约：晴朗，温度 20°C，湿度 45%".to_string(),
            "伦敦" | "london" => "伦敦：阴天，温度 15°C，湿度 70%".to_string(),
            "东京" | "tokyo" => "东京：多云，温度 22°C，湿度 60%".to_string(),
            _ => format!("{}：数据不可用", city),
        }
    }

    /// 获取多个城市的天气对比
    pub fn compare_weather(&self, cities: Vec<String>) -> String {
        let results: Vec<String> = cities
            .iter()
            .map(|city| format!("{}: {}", city, self.get_weather(city.clone())))
            .collect();
        results.join("\n")
    }
}

/// 时间工具
#[derive(Default, Clone)]
pub struct TimeService;

#[tool]
impl TimeService {
    /// 获取当前日期时间（格式：YYYY-MM-DD HH:MM:SS）
    pub fn get_current_time(&self) -> String {
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// 获取当前日期（格式：YYYY-MM-DD）
    pub fn get_current_date(&self) -> String {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    }

    /// 计算两个日期的天数差
    pub fn days_between(&self, date1: String, date2: String) -> Result<i32, String> {
        let d1 = chrono::NaiveDate::parse_from_str(&date1, "%Y-%m-%d")
            .map_err(|e| format!("日期格式错误：{}", e))?;
        let d2 = chrono::NaiveDate::parse_from_str(&date2, "%Y-%m-%d")
            .map_err(|e| format!("日期格式错误：{}", e))?;
        Ok((d2 - d1).num_days() as i32)
    }

    /// 格式化日期
    #[tool(example_format = "%Y/%m/%d")]
    pub fn format_date(&self, date: String, format: Option<String>) -> Result<String, String> {
        let d = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map_err(|e| format!("日期格式错误：{}", e))?;
        let fmt = format.as_deref().unwrap_or("%Y 年%m 月%d 日");
        Ok(d.format(fmt).to_string())
    }
}

// ==================== 工具调用处理器 ====================

/// 统一的工具调用处理器
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

    /// 处理工具调用
    fn handle_tool_call(&self, name: &str, args: &Value) -> Result<Value, String> {
        println!("   [工具调用] {}({:?})", name, args);

        let result = match name {
            // Calculator
            "add" | "multiply" | "sqrt" | "power" => self
                .calculator
                .call_tool(name, args)
                .map_err(|e| format!("计算器工具错误：{:?}", e))?,
            // HashCalculator
            "sha256" | "sha256_file" => self
                .hash_calculator
                .call_tool(name, args)
                .map_err(|e| format!("哈希工具错误：{:?}", e))?,
            // WeatherService
            "get_weather" | "compare_weather" => self
                .weather
                .call_tool(name, args)
                .map_err(|e| format!("天气工具错误：{:?}", e))?,
            // TimeService
            "get_current_time" | "get_current_date" | "days_between" | "format_date" => self
                .time_service
                .call_tool(name, args)
                .map_err(|e| format!("时间工具错误：{:?}", e))?,
            _ => return Err(format!("未知工具：{}", name)),
        };

        Ok(result)
    }

    /// 获取所有工具定义
    fn get_all_tools(&self) -> Vec<tokitai::ToolDefinition> {
        let mut tools = Vec::new();
        tools.extend(Calculator::tool_definitions().iter().cloned());
        tools.extend(HashCalculator::tool_definitions().iter().cloned());
        tools.extend(WeatherService::tool_definitions().iter().cloned());
        tools.extend(TimeService::tool_definitions().iter().cloned());
        tools
    }
}

// ==================== 主流程 ====================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_console();

    println!("=== Tokitai MCP Server 示例 ===\n");

    // 创建工具处理器
    let handler = Arc::new(ToolHandler::new());

    // 展示工具定义
    println!("已加载工具:");
    for tool in handler.get_all_tools() {
        println!("  - {}: {}", tool.name, tool.description);
    }
    println!();

    // 转换为 MCP 格式
    let mcp_tools = tokitai::mcp::to_mcp_tools(&handler.get_all_tools());
    println!("MCP 工具定义 ({} 个):", mcp_tools.len());
    for tool in &mcp_tools {
        println!(
            "  - {} ({} bytes)",
            tool.name,
            tool.input_schema.to_string().len()
        );
    }
    println!();

    // 演示工具调用
    println!("=== 工具调用演示 ===\n");

    // 1. 数学计算
    println!("[1] 数学计算");
    let result = handler.handle_tool_call("add", &json!({"a": 100, "b": 250}))?;
    println!("    add(100, 250) = {}\n", result);

    let result = handler.handle_tool_call("multiply", &json!({"a": 12, "b": 8}))?;
    println!("    multiply(12, 8) = {}\n", result);

    // 2. SHA256 计算
    println!("[2] SHA256 哈希计算");
    let result = handler.handle_tool_call("sha256", &json!({"input": "hello world"}))?;
    println!("    sha256('hello world') = {}\n", result);

    // 3. 天气查询
    println!("[3] 天气查询");
    let result = handler.handle_tool_call("get_weather", &json!({"city": "北京"}))?;
    println!("    get_weather('北京') = {}\n", result);

    // 4. 时间查询
    println!("[4] 时间查询");
    let result = handler.handle_tool_call("get_current_time", &json!({}))?;
    println!("    get_current_time() = {}\n", result);

    // 启动 HTTP 服务器的提示
    println!("=== 启动 HTTP 服务器 ===\n");
    println!("提示：要启动完整的 HTTP MCP 服务器，请运行:");
    println!("  cargo run --example mcp_http_server\n");
    println!("服务器端点:");
    println!("  GET  http://127.0.0.1:8080/tools  - 获取工具列表");
    println!("  POST http://127.0.0.1:8080/call   - 调用工具");
    println!("  GET  http://127.0.0.1:8080/health - 健康检查\n");

    // 展示如何与 AI 集成
    println!("=== 与 AI 集成示例 ===\n");
    println!("Python 示例:");
    println!(
        r#"
import requests

# 获取工具列表
response = requests.get("http://127.0.0.1:8080/tools")
tools = response.json()

# 调用工具
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
