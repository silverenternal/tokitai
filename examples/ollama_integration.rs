//! Ollama AI 集成示例
//!
//! 展示如何使用 tokitai 与 Ollama API 集成，实现 AI 工具调用功能
//!
//! # 前置要求
//!
//! 1. 安装 Ollama: https://ollama.ai
//! 2. 拉取模型：`ollama pull llama2` 或 `ollama pull mistral`
//! 3. 启动 Ollama 服务：`ollama serve`
//!
//! # 配置
//!
//! 复制 `examples/.env.example` 到 `examples/.env` 并配置：
//! ```text
//! # 本地 Ollama 服务（默认）
//! OLLAMA_BASE_URL=http://localhost:11434
//! OLLAMA_MODEL=llama2
//! OLLAMA_ENABLED=true
//! ```
//!
//! # 运行示例
//!
//! ```bash
//! # 从项目根目录运行
//! cargo run --example ollama_integration
//! ```

mod utils;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokitai::tool;
use tokitai::ToolProvider;
use sha2::{Sha256, Digest};
use utils::init_console;

// ==================== 环境变量配置 ====================

/// 从环境变量或默认值加载配置
struct Config {
    base_url: String,
    model: String,
    api_key: Option<String>,
    enabled: bool,
}

impl Config {
    fn from_env() -> Self {
        // 尝试从项目根目录加载 examples/.env
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let env_path = std::path::Path::new(&manifest_dir).join("examples").join(".env");
        dotenv::from_path(env_path).ok();
        
        // 也尝试当前目录的 .env（兼容）
        dotenv::dotenv().ok();

        let base_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "https://ollama.com".to_string());
        let model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "qwen3.5:cloud".to_string());
        let api_key = std::env::var("OLLAMA_API_KEY").ok();
        let enabled = std::env::var("OLLAMA_ENABLED")
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

/// 数学计算器工具
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

    /// 计算平方根（取整）
    pub fn sqrt(&self, n: i32) -> i32 {
        (n as f64).sqrt() as i32
    }
}

/// SHA256 哈希计算工具
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
pub struct WeatherService;

#[tool]
impl WeatherService {
    /// 获取指定城市的天气信息
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
            _ => format!("{}：数据不可用", city),
        }
    }
}

/// 时间工具
pub struct TimeService;

#[tool]
impl TimeService {
    /// 获取当前日期时间
    pub fn get_current_time(&self) -> String {
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// 计算两个日期的天数差
    pub fn days_between(&self, date1: String, date2: String) -> Result<i32, String> {
        let d1 = chrono::NaiveDate::parse_from_str(&date1, "%Y-%m-%d")
            .map_err(|e| format!("日期格式错误：{}", e))?;
        let d2 = chrono::NaiveDate::parse_from_str(&date2, "%Y-%m-%d")
            .map_err(|e| format!("日期格式错误：{}", e))?;
        Ok((d2 - d1).num_days() as i32)
    }
}

// ==================== Ollama API 类型定义 ====================

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

// ==================== Ollama 客户端 ====================

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

    /// 转换 tokitai 工具定义为 Ollama 格式
    #[allow(dead_code)]
    fn convert_tools<T: ToolProvider>(&self) -> Vec<ToolDefinition> {
        T::tool_definitions()
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

    /// 发送消息到 Ollama
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

        // Ollama 云 API 路径是 /api/chat
        let url = format!("{}/api/chat", self.base_url);
        
        let mut req = self.client.post(&url).json(&request);
        
        // 如果有 API key，添加认证头（Ollama 云需要）
        if let Some(ref api_key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = req.send().await
            .map_err(|e| format!("请求失败：{}", e))?;
        
        // 检查响应状态
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "未知错误".to_string());
            return Err(format!("API 返回错误 ({}): {}", status, error_text));
        }
        
        let result = response.json::<OllamaResponse>().await
            .map_err(|e| format!("解析响应失败：{}", e))?;

        Ok(result.message)
    }
}

// ==================== 主流程 ====================

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

    /// 处理工具调用
    fn handle_tool_call(
        &self,
        name: &str,
        args: &Value,
    ) -> Result<Value, String> {
        println!("   [工具调用] {}({:?})", name, args);

        // 根据工具名称路由到不同的服务
        let result = match name {
            "add" | "multiply" | "sqrt" => self.calculator.call_tool(name, args)
                .map_err(|e| format!("计算器工具错误：{:?}", e))?,
            "sha256" | "sha256_file" => self.hash_calculator.call_tool(name, args)
                .map_err(|e| format!("哈希工具错误：{:?}", e))?,
            "get_weather" => self.weather.call_tool(name, args)
                .map_err(|e| format!("天气工具错误：{:?}", e))?,
            "get_current_time" | "days_between" => self.time_service.call_tool(name, args)
                .map_err(|e| format!("时间工具错误：{:?}", e))?,
            _ => return Err(format!("未知工具：{}", name)),
        };

        Ok(result)
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    init_console();
    println!("=== Tokitai x Ollama AI 集成示例 ===\n");

    // 加载配置
    let config = Config::from_env();

    // 检查是否启用 Ollama 集成
    if !config.enabled {
        println!("ℹ️ OLLAMA_ENABLED=false，使用离线演示模式\n");
        println!("提示：要启用 Ollama 集成，请：");
        println!("  1. 安装 Ollama: https://ollama.ai");
        println!("  2. 拉取模型：ollama pull llama2");
        println!("  3. 启动服务：ollama serve");
        println!("  4. 复制 examples/.env.example 到 examples/.env");
        println!("  5. 设置 OLLAMA_ENABLED=true");
        println!("\n或者使用云端 AI 服务：");
        println!("  - 编辑 examples/.env，配置 OLLAMA_API_KEY 等");
        println!("  - 参考 docs/AI_INTEGRATION.md 了解如何集成其他 AI 服务\n");
        run_offline_demo().await.map_err(|e| e.to_string())?;
        return Ok(());
    }

    // 检查是否为在线服务（有 API key 或非 localhost 地址）
    let is_cloud = config.api_key.is_some() && !config.api_key.as_ref().unwrap().is_empty() || 
                   (!config.base_url.contains("localhost") && 
                    !config.base_url.contains("127.0.0.1"));
    
    if is_cloud {
        println!("✓ 使用 Ollama 云服务");
        println!("  服务地址：{}", config.base_url);
        println!("  模型：{}", config.model);
        if config.api_key.is_some() && !config.api_key.as_ref().unwrap().is_empty() {
            println!("  API Key: 已配置\n");
        } else {
            println!("  API Key: 未配置（可能需要）\n");
        }
    } else {
        // 本地服务检查
        println!("正在检查 Ollama 本地服务 ({})...", config.base_url);
        let client = reqwest::Client::new();
        match client.get(format!("{}/api/tags", config.base_url)).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    println!("✓ Ollama 本地服务正常运行\n");
                } else {
                    println!("⚠ Ollama 本地服务响应异常，继续示例演示...\n");
                }
            }
            Err(_) => {
                println!("⚠ Ollama 本地服务未运行，将展示离线演示模式\n");
                run_offline_demo().await.map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
    }

    // 创建助手和 Ollama 客户端
    let assistant = AiAssistant::new();
    let ollama = OllamaClient::new(&config);

    // 收集所有工具定义
    let mut all_tools = Vec::new();
    all_tools.extend(Calculator::TOOL_DEFINITIONS.iter().map(convert_tool_def));
    all_tools.extend(HashCalculator::TOOL_DEFINITIONS.iter().map(convert_tool_def));
    all_tools.extend(WeatherService::TOOL_DEFINITIONS.iter().map(convert_tool_def));
    all_tools.extend(TimeService::TOOL_DEFINITIONS.iter().map(convert_tool_def));

    println!("已加载 {} 个工具:", all_tools.len());
    for tool in &all_tools {
        println!("   - {}: {}", tool.function.name, tool.function.description);
    }
    println!();

    // 模拟对话 - 测试 SHA256 计算（AI 无法精确计算哈希值）
    let mut messages = vec![Message {
        role: "user".to_string(),
        content: "请计算字符串 'hello world' 的 SHA256 哈希值".to_string(),
        tool_calls: None,
    }];

    println!("[用户] 请计算字符串 'hello world' 的 SHA256 哈希值\n");

    // 第一轮对话
    let response = ollama
        .chat(messages.clone(), Some(all_tools.clone()))
        .await?;

    if let Some(tool_calls) = response.tool_calls {
        println!("[AI 请求工具调用]");
        for call in tool_calls {
            let result = assistant
                .handle_tool_call(&call.function.name, &call.function.arguments)?;
            println!("   [工具返回] {}\n", result);

            // 添加工具调用结果到消息
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

        // 获取最终回复
        let final_response = ollama.chat(messages, None).await
            .map_err(|e| e.to_string())?;
        println!("[AI 最终回复] {}", final_response.content);
    } else {
        println!("[AI 回复] {}", response.content);
    }

    Ok(())
}

/// 离线演示模式（当 Ollama 不可用时）
async fn run_offline_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 离线演示模式 ===\n");

    let assistant = AiAssistant::new();

    // 展示工具定义
    println!("1. 工具定义（可发送给任何 AI）");
    let tools = Calculator::TOOL_DEFINITIONS;
    for tool in tools {
        println!(
            "   {{ \"name\": \"{}\", \"description\": \"{}\", \"input_schema\": {} }}",
            tool.name, tool.description, tool.input_schema
        );
    }
    
    let hash_tools = HashCalculator::TOOL_DEFINITIONS;
    for tool in hash_tools {
        println!(
            "   {{ \"name\": \"{}\", \"description\": \"{}\", \"input_schema\": {} }}",
            tool.name, tool.description, tool.input_schema
        );
    }
    println!();

    // 模拟 AI 对话流程
    println!("2. 模拟 AI 对话流程");
    println!("   [用户] 请计算 100 + 250");
    println!("   [AI] 我来帮你计算...");

    let result = assistant
        .handle_tool_call("add", &json!({"a": 100, "b": 250}))?;
    println!("   [工具执行] {}", result);
    println!("   [AI] 结果是 {}\n", result);

    // SHA256 计算示例（AI 无法精确计算）
    println!("3. SHA256 哈希计算（AI 无法精确计算）");
    println!("   [用户] 计算 'hello world' 的 SHA256");
    println!("   [AI] 我来使用工具计算...");

    let result = assistant
        .handle_tool_call("sha256", &json!({"input": "hello world"}))?;
    println!("   [工具执行] {}", result);
    println!("   [AI] SHA256 哈希值是：{}\n", result);

    println!("4. 天气查询示例");
    println!("   [用户] 北京天气怎么样？");
    let result = assistant
        .handle_tool_call("get_weather", &json!({"city": "北京"}))?;
    println!("   [工具执行] {}", result);
    println!("   [AI] {}\n", result);

    println!("提示：安装并启动 Ollama 后可体验完整的 AI 集成功能");
    println!("   1. 访问 https://ollama.ai 下载安装");
    println!("   2. 运行：ollama pull llama2");
    println!("   3. 运行：ollama serve");
    println!("   4. 重新运行此示例");

    Ok(())
}

fn convert_tool_def(tool: &tokitai::ToolDefinition) -> ToolDefinition {
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
}
