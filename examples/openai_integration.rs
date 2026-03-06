//! OpenAI 兼容 API 集成示例
//!
//! 展示如何使用 tokitai 与 OpenAI API 或兼容服务（如 qwen3.5:cloud）集成
//!
//! # 配置
//!
//! 复制 `examples/.env.example` 到 `examples/.env` 并配置：
//! ```text
//! OPENAI_API_KEY=your-api-key-here
//! OPENAI_BASE_URL=https://api.openai.com/v1
//! OPENAI_MODEL=gpt-3.5-turbo
//! OPENAI_ENABLED=true
//! ```
//!
//! # 运行示例
//!
//! ```bash
//! cargo run --example openai_integration
//! ```

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokitai::tool;

// ==================== 环境变量配置 ====================

struct Config {
    base_url: String,
    model: String,
    api_key: String,
    enabled: bool,
}

impl Config {
    fn from_env() -> Self {
        dotenv::dotenv().ok();

        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let api_key = std::env::var("OPENAI_API_KEY")
            .unwrap_or_else(|_| String::new());
        let model = std::env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-3.5-turbo".to_string());
        let enabled = std::env::var("OPENAI_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true";

        Self {
            base_url,
            model,
            api_key,
            enabled,
        }
    }
}

// ==================== 工具定义 ====================

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

    /// 计算平方根
    pub fn sqrt(&self, n: i32) -> i32 {
        (n as f64).sqrt() as i32
    }
}

pub struct WeatherService;

#[tool]
impl WeatherService {
    /// 获取指定城市的天气信息
    pub fn get_weather(&self, city: String) -> String {
        match city.to_lowercase().as_str() {
            "北京" | "beijing" => "北京：晴朗，温度 25°C，湿度 40%".to_string(),
            "上海" | "shanghai" => "上海：多云，温度 22°C，湿度 60%".to_string(),
            "广州" | "guangzhou" => "广州：小雨，温度 28°C，湿度 80%".to_string(),
            _ => format!("{}：天气晴朗，温度 26°C", city),
        }
    }
}

// ==================== OpenAI API 类型定义 ====================

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    tool_choice: Option<String>,
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
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ToolCallResponse {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: FunctionCall,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FunctionCall {
    name: String,
    arguments: String,
}

// ==================== OpenAI 客户端 ====================

struct OpenAIClient {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl OpenAIClient {
    fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            api_key: config.api_key.clone(),
        }
    }

    fn convert_tools(&self, tool_defs: &[tokitai::ToolDefinition]) -> Vec<ToolDefinition> {
        tool_defs
            .iter()
            .map(|tool| {
                let schema: Value = serde_json::from_str(tool.input_schema).unwrap_or_else(|_| {
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

    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<Message, String> {
        let request = OpenAIRequest {
            model: self.model.clone(),
            messages,
            tools,
            tool_choice: None,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self.client.post(&url)
            .json(&request)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| format!("请求失败：{}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "未知错误".to_string());
            return Err(format!("API 返回错误：{}", error_text));
        }

        let result = response.json::<OpenAIResponse>().await
            .map_err(|e| format!("解析响应失败：{}", e))?;

        result.choices.into_iter()
            .next()
            .map(|c| c.message)
            .ok_or_else(|| "没有返回消息".to_string())
    }
}

// ==================== 主流程 ====================

struct AiAssistant {
    calculator: Calculator,
    weather: WeatherService,
}

impl AiAssistant {
    fn new() -> Self {
        Self {
            calculator: Calculator,
            weather: WeatherService,
        }
    }

    fn handle_tool_call(&self, name: &str, args: &Value) -> Result<Value, String> {
        println!("   [工具调用] {}({:?})", name, args);

        let result = match name {
            "add" | "multiply" | "sqrt" => self.calculator.call_tool(name, args)
                .map_err(|e| format!("计算器工具错误：{:?}", e))?,
            "get_weather" => self.weather.call_tool(name, args)
                .map_err(|e| format!("天气工具错误：{:?}", e))?,
            _ => return Err(format!("未知工具：{}", name)),
        };

        Ok(result)
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("=== OpenAI 兼容 API 集成示例 ===\n");

    let config = Config::from_env();

    if !config.enabled {
        println!("ℹ️ OPENAI_ENABLED=false，使用离线演示模式\n");
        println!("提示：要启用 OpenAI 兼容 API 集成，请：");
        println!("  1. 复制 examples/.env.example 到 examples/.env");
        println!("  2. 设置 OPENAI_API_KEY=your-api-key");
        println!("  3. 设置 OPENAI_BASE_URL=https://api.openai.com/v1 (或其他兼容地址)");
        println!("  4. 设置 OPENAI_MODEL=your-model");
        println!("  5. 设置 OPENAI_ENABLED=true\n");
        run_offline_demo().await;
        return Ok(());
    }

    println!("✓ 使用 OpenAI 兼容 API");
    println!("  服务地址：{}", config.base_url);
    println!("  模型：{}", config.model);
    println!("  API Key: 已配置\n");

    let assistant = AiAssistant::new();
    let openai = OpenAIClient::new(&config);

    let mut all_tools = Vec::new();
    all_tools.extend(openai.convert_tools(&Calculator::TOOL_DEFINITIONS));
    all_tools.extend(openai.convert_tools(&WeatherService::TOOL_DEFINITIONS));

    println!("已加载 {} 个工具:", all_tools.len());
    for tool in &all_tools {
        println!("   - {}: {}", tool.function.name, tool.function.description);
    }
    println!();

    let mut messages = vec![Message {
        role: "user".to_string(),
        content: "北京今天的天气怎么样？".to_string(),
        tool_calls: None,
    }];

    println!("[用户] 北京今天的天气怎么样？\n");

    let response = openai.chat(messages.clone(), Some(all_tools.clone())).await?;

    if let Some(ref tool_calls) = response.tool_calls {
        println!("[AI 请求工具调用]");
        for call in tool_calls {
            let args: Value = serde_json::from_str(&call.function.arguments)
                .unwrap_or_else(|_| json!({}));
            let result = assistant.handle_tool_call(&call.function.name, &args)?;
            println!("   [工具返回] {}\n", result);

            messages.push(Message {
                role: "assistant".to_string(),
                content: "".to_string(),
                tool_calls: Some(vec![call.clone()]),
            });
            messages.push(Message {
                role: "tool".to_string(),
                content: result.to_string(),
                tool_calls: None,
            });
        }

        let final_response = openai.chat(messages, None).await?;
        println!("[AI 最终回复] {}", final_response.content);
    } else {
        println!("[AI 回复] {}", response.content);
    }

    Ok(())
}

async fn run_offline_demo() {
    println!("=== 离线演示模式 ===\n");

    let assistant = AiAssistant::new();

    println!("1. 工具定义（可发送给任何 AI）");
    let tools = Calculator::TOOL_DEFINITIONS;
    for tool in tools {
        println!(
            "   {{ \"name\": \"{}\", \"description\": \"{}\", \"input_schema\": {} }}",
            tool.name, tool.description, tool.input_schema
        );
    }
    println!();

    println!("2. 模拟 AI 工具调用");
    println!("   [用户] 请计算 100 + 250");
    let result = assistant.handle_tool_call("add", &json!({"a": 100, "b": 250})).unwrap();
    println!("   [工具执行] {}", result);
    println!("   [AI] 结果是 {}\n", result);

    println!("提示：配置 API key 后可体验完整的 AI 集成功能");
}
