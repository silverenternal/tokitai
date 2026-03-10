# Tokitai AI 集成指南

**版本**: 0.4.0 | **最后更新**: 2026-03-10

## 目录

1. [概述](#概述)
2. [与 Ollama 集成](#与-ollama-集成)
3. [与其他 AI 平台集成](#与其他-ai-平台集成)
4. [完整工作流程](#完整工作流程)
5. [故障排除](#故障排除)

---

## 概述

Tokitai 的核心设计理念是**供应商中立**。宏生成的工具定义可以被发送给任何支持工具/函数调用的 AI 平台。

### 工作流程

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  获取工具定义  │ ──> │  发送给 AI   │ ──> │  接收调用请求 │
│  (编译期生成) │     │  (JSON 格式)  │     │  (解析参数)   │
└─────────────┘     └─────────────┘     └─────────────┘
                                              │
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  返回最终响应  │ <── │  执行工具    │ <── │  调用 Rust   │
│  (给 AI)      │     │  (获取结果)  │     │  (执行业务)  │
└─────────────┘     └─────────────┘     └─────────────┘
```

---

## 与 Ollama 集成

### 前置要求

1. **安装 Ollama**
   ```bash
   # macOS/Linux
   curl -fsSL https://ollama.ai/install.sh | sh
   
   # Windows: 访问 https://ollama.ai 下载安装程序
   ```

2. **拉取模型**
   ```bash
   ollama pull llama2
   # 或者
   ollama pull mistral
   # 或者（支持工具调用的模型）
   ollama pull llama3.1
   ```

3. **启动服务**
   ```bash
   ollama serve
   ```

### 完整示例

```rust
use tokitai::tool;
use tokitai::ToolProvider;
use serde_json::json;

// 1. 定义工具
#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// 2. 转换工具定义为 Ollama 格式
fn convert_to_ollama_format(tools: &[ToolDefinition]) -> Vec<Value> {
    tools.iter().map(|tool| {
        json!({
            "type": "function",
            "function": {
                "name": tool.name,
                "description": tool.description,
                "parameters": serde_json::from_str::<Value>(tool.input_schema).unwrap()
            }
        })
    }).collect()
}

// 3. 发送请求到 Ollama
async fn chat_with_ollama(messages: Vec<Message>, tools: Vec<Value>) -> Result<Message, Error> {
    let client = reqwest::Client::new();
    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&json!({
            "model": "llama3.1",
            "messages": messages,
            "tools": tools,
            "stream": false
        }))
        .send()
        .await?
        .json::<OllamaResponse>()
        .await?;
    
    Ok(response.message)
}

// 4. 处理工具调用
async fn handle_tool_call(assistant: &Assistant, call: &ToolCall) -> Value {
    assistant.call_tool(&call.function.name, &call.function.arguments).await
}
```

### 运行示例

```bash
# 运行完整的 Ollama 集成示例
cargo run --example ollama_integration
```

---

## 与其他 AI 平台集成

### Claude API

```rust
use serde_json::json;

// 转换工具定义为 Claude 格式
fn convert_to_claude_format(tools: &[ToolDefinition]) -> Vec<Value> {
    tools.iter().map(|tool| {
        json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": serde_json::from_str::<Value>(tool.input_schema).unwrap()
        })
    }).collect()
}

// 发送请求到 Claude
async fn chat_with_claude(messages: Vec<Message>, tools: Vec<Value>) -> Result<Message, Error> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", std::env::var("ANTHROPIC_API_KEY")?)
        .json(&json!({
            "model": "claude-3-sonnet-20240229",
            "max_tokens": 1024,
            "messages": messages,
            "tools": tools
        }))
        .send()
        .await?
        .json::<ClaudeResponse>()
        .await?;
    
    Ok(response.content)
}
```

### OpenAI GPT

```rust
use serde_json::json;

// 转换工具定义为 OpenAI 格式
fn convert_to_openai_format(tools: &[ToolDefinition]) -> Vec<Value> {
    tools.iter().map(|tool| {
        json!({
            "type": "function",
            "function": {
                "name": tool.name,
                "description": tool.description,
                "parameters": serde_json::from_str::<Value>(tool.input_schema).unwrap()
            }
        })
    }).collect()
}

// 发送请求到 OpenAI
async fn chat_with_openai(messages: Vec<Message>, tools: Vec<Value>) -> Result<Message, Error> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", std::env::var("OPENAI_API_KEY")?))
        .json(&json!({
            "model": "gpt-4-turbo",
            "messages": messages,
            "tools": tools
        }))
        .send()
        .await?
        .json::<OpenAIResponse>()
        .await?;
    
    Ok(response.choices[0].message.clone())
}
```

### MCP (Model Context Protocol)

```rust
use tokitai::{tool, mcp};

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// 转换为 MCP 格式
let mcp_tools = mcp::to_mcp_tools(Calculator::tool_definitions());

// MCP 工具格式
// [
//   {
//     "name": "add",
//     "description": "两个数相加",
//     "input_schema": {"type": "object", "properties": {...}}
//   }
// ]
```

---

## 完整工作流程

### 步骤 1: 准备工具定义

```rust
use tokitai::{tool, ToolProvider};

#[tool]
impl WeatherService {
    /// 获取指定城市的天气
    pub fn get_weather(&self, city: String) -> String {
        // 业务逻辑...
    }
}

let tools = WeatherService::tool_definitions();
println!("工具数量：{}", tools.len());
```

### 步骤 2: 发送给 AI

```rust
let system_message = Message {
    role: "system".to_string(),
    content: "你是一个有帮助的助手。使用可用的工具来回答用户的问题。".to_string(),
};

let user_message = Message {
    role: "user".to_string(),
    content: "北京今天的天气怎么样？".to_string(),
};

let tools_json = tools.iter().map(|t| {
    json!({
        "type": "function",
        "function": {
            "name": t.name,
            "description": t.description,
            "parameters": serde_json::from_str::<Value>(t.input_schema).unwrap()
        }
    })
}).collect::<Vec<_>>();

let response = call_ai_api(vec![system_message, user_message], tools_json).await?;
```

### 步骤 3: 执行工具调用

```rust
let assistant = WeatherService;

if let Some(tool_calls) = response.tool_calls {
    for call in tool_calls {
        let result = assistant
            .call_tool(&call.function.name, &call.function.arguments)
            .await?;
        
        println!("工具 {} 返回：{}", call.function.name, result);
    }
}
```

### 步骤 4: 返回结果给 AI

```rust
// 添加工具调用结果到消息历史
messages.push(Message {
    role: "assistant".to_string(),
    content: "".to_string(),
    tool_calls: Some(tool_calls),
});

for (call, result) in tool_calls.iter().zip(results.iter()) {
    messages.push(Message {
        role: "tool".to_string(),
        content: result.to_string(),
        tool_calls: None,
    });
}

// 获取最终回复
let final_response = call_ai_api(messages, None).await?;
println!("AI 回复：{}", final_response.content);
```

---

## 故障排除

### Ollama 服务未运行

```bash
# 检查服务状态
curl http://localhost:11434/api/tags

# 启动服务
ollama serve
```

### 模型不支持工具调用

某些模型可能不支持工具调用功能。使用以下模型：

- ✅ Ollama: `llama3.1`, `mistral`, `mixtral`
- ✅ Claude: `claude-3-*` 系列
- ✅ GPT: `gpt-3.5-turbo`, `gpt-4-*` 系列

### 工具定义格式错误

确保 JSON Schema 格式正确：

```rust
// 正确的格式
{"type":"object","properties":{"a":{"type":"integer","description":""},"b":{"type":"integer","description":""}},"required":["a","b"]}

// 检查工具定义
for tool in Calculator::tool_definitions() {
    println!("{}: {}", tool.name, tool.input_schema);
}
```

### 参数类型不匹配

确保 Rust 类型与 JSON 类型匹配：

| Rust | JSON |
|------|------|
| `i32`, `i64` | `integer` |
| `f32`, `f64` | `number` |
| `String` | `string` |
| `bool` | `boolean` |
| `Vec<T>` | `array` |

---

## 示例代码

- [`examples/ollama_integration.rs`](../examples/ollama_integration.rs) - 完整的 Ollama 集成示例
- [`examples/multi_tool_chat.rs`](../examples/multi_tool_chat.rs) - 多工具协作示例

---

## 参考资源

- [Ollama API 文档](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [Claude API 文档](https://docs.anthropic.com/claude/docs)
- [OpenAI Function Calling](https://platform.openai.com/docs/guides/function-calling)
- [MCP 协议](https://modelcontextprotocol.io/)
