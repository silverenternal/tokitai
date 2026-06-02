# Tokitai x MCP 架构指南

**版本**: 0.5.0
**最后更新**: 2026-06-02

---

## 📖 概述

本指南介绍如何使用 Tokitai 构建基于 MCP (Model Context Protocol) 的 AI 工具服务器。

### 核心理念

> **编译期生成、零运行时侵入、类型安全**

Tokitai 的核心理念与 MCP 协议完美结合，让 Rust 成为编写"AI 原生后端"的最佳语言。

---

## 🏗️ 架构设计

### 整体架构

```
┌─────────────────┐         ┌─────────────────────┐         ┌─────────────────┐
│   AI Client     │         │  MCP Server         │         │  Business Logic │
│   (Python/JS)   │ ──────> │  (tokitai-mcp)      │ ──────> │  (Rust tools)   │
│                 │ <────── │                     │ <────── │  #[tool]        │
└─────────────────┘         └─────────────────────┘         └─────────────────┘
     │                           │                              │
     │ 1. List tools             │                              │
     │ 2. Call tool (JSON)       │                              │
     │                           │ 3. Type-safe call            │
     │                           │                              │
     │ 4. Result (JSON)          │                              │
```

### 组件说明

| 组件 | 角色 | 关键技术 |
|------|------|----------|
| **AI Client** | 轻量化决策者 | 只发送 JSON 请求，不加载业务代码 |
| **MCP Server** | 编译时处理中心 | tokitai 宏生成工具定义 |
| **Business Logic** | 强类型内核 | `#[tool]` 标记的 Rust 代码 |

---

## 🚀 快速开始

### 1. 添加依赖

```toml
[dependencies]
tokitai = { version = "0.5.0", features = ["mcp"] }
tokitai-mcp-server = "0.5"  # 可选：MCP 服务器脚手架
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
```

### 2. 定义工具

```rust
use tokitai::tool;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 计算 SHA256 哈希值
    pub fn sha256(&self, input: String) -> String {
        // 你的业务逻辑
        format!("hash of {}", input)
    }
}
```

### 3. 获取工具定义

```rust
// 编译期生成的工具定义
let tools = Calculator::tool_definitions();

// 转换为 MCP 格式
let mcp_tools = tokitai::mcp::to_mcp_tools(&tools);

// 发送给 AI
let tools_json = serde_json::to_string_pretty(&mcp_tools)?;
```

### 4. 处理 AI 调用

```rust
use serde_json::json;

let calc = Calculator::default();

// AI 决定调用工具
let call_request = json!({
    "name": "add",
    "arguments": {"a": 10, "b": 20}
});

// 执行工具（类型安全）
let result = calc.call_tool(
    call_request["name"].as_str().unwrap(),
    &call_request["arguments"]
)?;

println!("{}", result);  // 30
```

---

## 📦 完整示例：MCP HTTP 服务器

### 使用 tokitai-mcp-server 脚手架

```rust
use tokitai::tool;
use tokitai_mcp_server::McpServerBuilder;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[tokio::main]
async fn main() {
    let server = McpServerBuilder::with_tool(Calculator::default())
        .with_port(8080)
        .build();

    server.run().await.unwrap();
}
```

### 自定义 HTTP 服务器

```rust
use axum::{routing::{get, post}, Json, Router};
use tokitai::{tool, mcp::to_mcp_tools, ToolProvider};
use serde_json::json;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[tokio::main]
async fn main() {
    // 获取工具定义
    let tools = to_mcp_tools(&Calculator::tool_definitions());
    
    // 构建路由
    let app = Router::new()
        .route("/tools", get(|| async { tools }))
        .route("/call", post(|body: Json<Value>| async {
            // 处理工具调用
            json!({"success": true, "result": 30})
        }));
    
    // 启动服务器
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

---

## 🔧 运行示例

### 示例 1：基础 MCP 服务器演示

```bash
# 从项目根目录运行
cargo run --example mcp_server_demo
```

输出示例：
```
=== Tokitai MCP Server 示例 ===

已加载工具:
  - add: 两个数相加
  - multiply: 两个数相乘
  - sqrt: 计算平方根
  - sha256: 计算字符串的 SHA256 哈希值
  - get_weather: 获取指定城市的天气信息
  - get_current_time: 获取当前日期时间

MCP 工具定义 (6 个):
  - add (150 bytes)
  - multiply (140 bytes)
  ...

=== 工具调用演示 ===

[1] 数学计算
   [工具调用] add({"a": 100, "b": 250})
    add(100, 250) = 350
```

### 示例 2：HTTP 服务器

```bash
# 启动 HTTP 服务器
cargo run --example mcp_http_server

# 在另一个终端测试 API
# 获取工具列表
curl http://127.0.0.1:8080/tools

# 调用工具
curl -X POST http://127.0.0.1:8080/call \
  -H "Content-Type: application/json" \
  -d '{"name": "add", "arguments": {"a": 10, "b": 20}}'
```

---

## 🌐 AI 客户端集成

### Python 示例

```python
import requests

# 获取工具列表
response = requests.get("http://127.0.0.1:8080/tools")
tools = response.json()
print(f"Available tools: {len(tools)}")

# 调用工具
response = requests.post(
    "http://127.0.0.1:8080/call",
    json={"name": "add", "arguments": {"a": 10, "b": 20}}
)
result = response.json()
print(f"Result: {result['result']}")  # 30
```

### JavaScript 示例

```javascript
// 获取工具列表
const toolsResponse = await fetch('http://127.0.0.1:8080/tools');
const tools = await toolsResponse.json();

// 调用工具
const callResponse = await fetch('http://127.0.0.1:8080/call', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    name: 'add',
    arguments: { a: 10, b: 20 }
  })
});
const result = await callResponse.json();
console.log(`Result: ${result.result}`);  // 30
```

---

## 🎯 架构优势

### 1. 轻量化 (Lightweight)

| 层面 | 优势 |
|------|------|
| **Agent 端** | 无业务代码，上下文精简 |
| **传输层** | JSON 序列化，数据量最小化 |
| **运行时** | 零解释器开销，原生执行 |

### 2. 强编译时处理 (Strong Compile-time)

| 特性 | 实现方式 |
|------|----------|
| **Schema 生成** | 过程宏编译期生成，非运行时反射 |
| **类型检查** | Rust 类型系统保证参数匹配 |
| **错误捕获** | 编译期发现类型错误 |

### 3. MCP 灵活性

| 能力 | 说明 |
|------|------|
| **语言无关** | Agent 可是 Python/JS/任意语言 |
| **协议标准** | 遵循 MCP 协议规范 |
| **可扩展** | 轻松添加新工具 |

---

## 📋 类型映射

| Rust 类型 | JSON Schema | 示例 |
|-----------|-------------|------|
| `String`, `&str` | `string` | `"hello"` |
| `i32`, `i64`, `u32` | `integer` | `42` |
| `f32`, `f64` | `number` | `3.14` |
| `bool` | `boolean` | `true` |
| `Vec<T>` | `array` | `[1, 2, 3]` |
| 自定义 struct | `object` | `{"name": "Alice"}` |

---

## 🔐 类型安全保证

### 编译期检查

```rust
#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// 如果 AI 传入错误的参数类型，call_tool 会在运行时返回错误
// 但 Rust 类型系统在编译期就保证了函数签名的正确性
```

### 运行时验证

```rust
let calc = Calculator::default();

// Parameter type mismatch returns an error
let result = calc.call_tool("add", &json!({
    "a": "not a number",  // wrong: should be an integer
    "b": 20
}));

assert!(result.is_err());
```

---

## 🛠️ 最佳实践

### 1. 工具设计

```rust
#[tool]
impl MyTools {
    /// 清晰的文档注释（自动成为 AI 工具描述）
    #[tool(tags = ["category", "feature"])]
    pub fn process_data(
        &self,
        #[param_tool(desc = "输入数据", example = "sample")]
        input: String,
        
        #[param_tool(desc = "处理选项", default = "null")]
        options: Option<Vec<String>>,
    ) -> Result<String, MyError> {
        // 业务逻辑
    }
}
```

### 2. 错误处理

```rust
use tokitai::AiToolError;

#[tool]
impl MyTools {
    pub fn risky_operation(&self, data: String) -> Result<String, AiToolError> {
        // 验证输入
        if data.is_empty() {
            return Err(AiToolError::validation_error("Data cannot be empty"));
        }
        
        // 业务逻辑
        Ok(format!("Processed: {}", data))
    }
}
```

### 3. 性能优化

```rust
// ✅ 推荐：使用默认特征
#[derive(Default, Clone)]
struct MyTools;

// ✅ 推荐：工具实例复用
let tools = MyTools::default();
let result1 = tools.call_tool("op1", &args1)?;
let result2 = tools.call_tool("op2", &args2)?;
```

---

## 📚 相关资源

- [5 分钟快速开始](quickstart.md)
- [高级用法](ADVANCED_USAGE.md)
- [类型系统](USAGE.md)
- [AI 集成](AI_INTEGRATION.md)
- [API 文档](https://docs.rs/tokitai)

---

## ❓ 常见问题

### Q: MCP 服务器必须用 `tokitai-mcp-server` 吗？

A: 不是必须的。`tokitai-mcp-server` 只是一个可选的脚手架，你可以用任何 HTTP 框架（如 axum、actix-web）自行构建服务器。

### Q: 如何在运行时动态添加工具？

A: Tokitai 的工具定义在编译期生成，不支持运行时动态添加。如需动态工具，考虑使用多个 `#[tool]` 类型组合。

### Q: 支持异步工具吗？

A: 支持。启用 `runtime` 特征后，可以定义 `async fn` 工具。

```rust
#[tool]
impl AsyncTools {
    pub async fn fetch_url(&self, url: String) -> String {
        reqwest::get(&url).await.unwrap().text().await.unwrap()
    }
}
```

---

**Happy Coding!** 🦀
