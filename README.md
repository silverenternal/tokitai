# Tokitai

[![Crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai)
[![Documentation](https://docs.rs/tokitai/badge.svg)](https://docs.rs/tokitai)
[![License](https://img.shields.io/crates/l/tokitai)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/silverenternal/tokitai/ci.yml)](https://github.com/silverenternal/tokitai/actions)

**编译期 AI 工具定义 · 零运行时侵入 · 魔法贴纸式集成**

Tokitai 是一个零运行时依赖的过程宏库，只需一个 `#[tool]` 属性，即可将你的 Rust 方法自动转换为 AI 可调用的工具。所有工具定义在编译期生成，类型错误在编译时暴露。

## 为什么选择 Tokitai？

LLM 无法精确执行某些任务（如数学计算、文件操作、API 调用）。Tokitai 让你能够：

1. **定义 Rust 方法** → 实现你的业务逻辑
2. **发送给 AI** → AI 知道有哪些工具可用
3. **接收调用请求** → AI 返回"我想调用某个工具"
4. **执行并返回结果** → 本地执行 Rust 代码

```
┌─────────────┐    工具定义    ┌─────────────┐
│   你的代码   │ ────────────→ │   AI 服务    │
│  #[tool]    │               │ (Ollama 等)  │
└─────────────┘               └─────────────┘
       ↑                             │
       │ 执行结果                    │ 调用请求
       │                             ↓
┌─────────────┐               ┌─────────────┐
│  Rust 方法   │ ←──────────── │  JSON 调用  │
│  本地执行   │   call_tool   │  {"name":..}│
└─────────────┘               └─────────────┘
```

## 快速开始

### 1. 添加依赖

```toml
[dependencies]
tokitai = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 2. 定义工具

```rust
use tokitai::tool;

pub struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 计算字符串的 SHA256 哈希值
    pub fn sha256(&self, input: String) -> String {
        // 你的实现...
        format!("hash of {}", input)
    }
}
```

### 3. 获取工具定义（发送给 AI）

```rust
// 编译期生成的工具定义
let tools = Calculator::TOOL_DEFINITIONS;

// 转换为 JSON 发送给 AI
let tools_json = serde_json::to_string_pretty(tools)?;
println!("{}", tools_json);
```

输出：
```json
[
  {
    "name": "add",
    "description": "两个数相加",
    "input_schema": "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}"
  },
  {
    "name": "sha256",
    "description": "计算字符串的 SHA256 哈希值",
    "input_schema": "{\"type\":\"object\",\"properties\":{\"input\":{\"type\":\"string\"}},\"required\":[\"input\"]}"
  }
]
```

### 4. 处理 AI 调用

```rust
use serde_json::json;

let calc = Calculator;

// AI 请求调用 add 工具
let ai_request = json!({"name": "add", "arguments": {"a": 10, "b": 20}});

// 执行工具调用
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}))?;
assert_eq!(result.as_i64().unwrap(), 30);
```

## 核心特性

| 特性 | 描述 |
|------|------|
| 🏷️ **零学习成本** | 只需 `#[tool]`，无需学习复杂 API |
| 🔒 **编译期生成** | 工具定义在编译期生成，类型安全 |
| 🪶 **零运行时依赖** | `default-features = false` 仅依赖 `serde` |
| 🔌 **供应商中立** | 生成的工具定义兼容任何支持 Function Calling 的 AI |
| 📦 **MCP 兼容** | 可选 Model Context Protocol 支持 |
| 🚫 **灵活排除** | `#[tool(skip)]` 排除内部方法 |
| ⚡ **异步支持** | 完整支持 `async fn` |

## 完整示例

```rust
use tokitai::tool;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct WeatherData {
    pub temperature: f64,
    pub condition: String,
}

pub struct WeatherService;

#[tool]
impl WeatherService {
    /// 获取指定城市的实时天气
    pub fn get_weather(&self, city: String) -> Result<WeatherData, String> {
        // 你的业务逻辑或 API 调用
        Ok(WeatherData {
            temperature: 25.0,
            condition: "晴朗".to_string(),
        })
    }

    /// 获取未来 N 天的天气预报
    pub fn get_forecast(&self, city: String, days: Option<i32>) -> Vec<String> {
        let days = days.unwrap_or(3);
        (0..days).map(|i| format!("第 {} 天：晴朗", i + 1)).collect()
    }

    // 内部方法，不暴露给 AI
    #[tool(skip)]
    fn fetch_from_api(&self, endpoint: &str) -> Result<String, String> {
        // 内部实现细节
        Ok("API response".to_string())
    }
}

// 使用
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = WeatherService;

    // 1. 获取工具定义
    println!("可用工具：");
    for tool in WeatherService::TOOL_DEFINITIONS {
        println!("  - {}: {}", tool.name, tool.description);
    }

    // 2. 处理 AI 调用
    use serde_json::json;
    
    let result = service.call_tool("get_weather", &json!({"city": "北京"}))?;
    println!("天气数据：{}", result);

    Ok(())
}
```

## 支持的参数类型

| Rust 类型 | JSON Schema | 示例 |
|-----------|-------------|------|
| `String`, `&str` | `string` | `"hello"` |
| `i8`..=`i128`, `u8`..=`u128` | `integer` | `42` |
| `f32`, `f64` | `number` | `3.14` |
| `bool` | `boolean` | `true` |
| `Vec<T>` | `array` | `[1, 2, 3]` |
| `Option<T>` | 可选参数 | `null` 或值 |
| 自定义类型 | `object` | `{"field": "value"}` |

## 高级用法

### 自定义工具名称和描述

```rust
#[tool]
impl Calculator {
    #[tool(name = "add_numbers", desc = "将两个数字相加并返回结果")]
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

### 异步方法

```rust
#[tool]
impl Database {
    pub async fn query(&self, sql: String) -> Result<Vec<Row>, DbError> {
        // 异步数据库查询
    }
}
```

### 复杂参数类型

```rust
#[derive(Debug, Deserialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

#[tool]
impl MapService {
    pub fn get_weather_at(&self, location: Location) -> String {
        format!("位置 ({}, {}) 的天气", location.latitude, location.longitude)
    }
}
```

## 与 AI 平台集成

### Ollama

```rust
// 1. 发送工具定义给 Ollama
let tools = Calculator::TOOL_DEFINITIONS;
let request = serde_json::json!({
    "model": "qwen3.5:397b",
    "messages": [{"role": "user", "content": "计算 100 + 250"}],
    "tools": tools
});

// 2. 调用 Ollama API
let response = reqwest::Client::new()
    .post("http://localhost:11434/api/chat")
    .json(&request)
    .send()
    .await?;

// 3. AI 返回工具调用请求
// {"message": {"tool_calls": [{"function": {"name": "add", "arguments": {"a": 100, "b": 250}}}]}}

// 4. 执行工具调用
let calc = Calculator;
let result = calc.call_tool("add", &json!({"a": 100, "b": 250}))?;

// 5. 返回结果给 AI，获取最终回复
```

完整示例请查看 [`examples/ollama_integration.rs`](examples/ollama_integration.rs)

## 项目结构

```
tokitai/
├── tokitai-core/     # 核心类型定义（零依赖）
├── tokitai-macros/   # 过程宏（编译期代码生成）
├── tokitai/          # 运行时库（可选）
├── examples/         # 使用示例
│   ├── basic_usage.rs
│   └── ollama_integration.rs
└── docs/             # 详细文档
    ├── USAGE.md
    ├── AI_INTEGRATION.md
    └── ARCHITECTURE.md
```

## 文档

| 文档 | 描述 |
|------|------|
| [📖 使用指南](docs/USAGE.md) | 详细 API 和高级功能 |
| [🤖 AI 集成指南](docs/AI_INTEGRATION.md) | 与 Ollama 等平台集成 |
| [🏛️ 架构设计](docs/ARCHITECTURE.md) | 内部设计和宏展开说明 |
| [📝 API 参考](https://docs.rs/tokitai) | Rust API 文档 |

## 运行示例

```bash
# 基础使用示例
cargo run --example basic_usage

# Ollama 集成示例
cargo run --example ollama_integration

# 运行测试
cargo test --workspace

# 查看宏展开代码
cargo expand --example basic_usage
```

## 许可证

本项目采用双许可：

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

## 贡献

欢迎提交 Issue 和 Pull Request！详情请查看 [贡献指南](CONTRIBUTING.md)。

## 版本历史

查看 [CHANGELOG.md](CHANGELOG.md) 了解完整版本历史。

## 相关链接

- **GitHub**: https://github.com/silverenternal/tokitai
- **Crates.io**: https://crates.io/crates/tokitai
- **docs.rs**: https://docs.rs/tokitai

---

<div align="center">

**Tokitai** · 让 AI 调用你的 Rust 代码从未如此简单

</div>
