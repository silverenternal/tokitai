# Tokitai

**编译期 AI 工具定义，零运行时侵入**

Tokitai 是一个 Rust 库，让你只需一个 `#[tool]` 标签就能将 Rust 方法暴露给 AI 调用。

## 快速开始

```rust
use tokitai::tool;
use serde_json::json;

pub struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub async fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 两个数相乘
    pub async fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}

// 使用
#[tokio::main]
async fn main() {
    let calc = Calculator;

    // 获取工具列表（编译期生成）
    let tools = Calculator::TOOL_DEFINITIONS;
    
    // 调用工具
    let result = calc
        .call_tool("add", &json!({"a": 10, "b": 20}))
        .await
        .unwrap();
    
    assert_eq!(result.as_i64().unwrap(), 30);
}
```

## 特性

- ✅ **零运行时侵入** - 宏本身零依赖，不强制绑定任何运行时
- ✅ **编译期类型安全** - 工具定义在编译期生成，参数类型错误编译时暴露
- ✅ **单一宏** - 只需 `#[tool]`，无需同时贴多个标签
- ✅ **可选运行时** - 通过 features 控制依赖，支持无异步环境

## 安装

```toml
[dependencies]
tokitai = "0.2"
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
```

### 最小化依赖

如果只需要编译期工具定义：

```toml
[dependencies]
tokitai = { version = "0.2", default-features = false }
tokitai-core = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## 使用指南

### 1. 基础用法

在 impl 块上添加 `#[tool]` 属性，所有 `pub async fn` 方法会自动注册为工具：

```rust
use tokitai::tool;

#[tool]
impl MyService {
    /// 获取用户信息
    pub async fn get_user(&self, user_id: String) -> User {
        // ...
    }
}
```

### 2. 自定义工具属性

使用 `#[tool(name = "...", desc = "...")]` 自定义工具名称和描述：

```rust
#[tool]
impl MyService {
    #[tool(name = "fetch_user", desc = "从数据库获取用户信息")]
    pub async fn get_user(&self, user_id: String) -> User {
        // ...
    }
}
```

### 3. 参数类型支持

支持以下参数类型：

- 基本类型：`String`, `i32/i64`, `f32/f64`, `bool`
- 可选类型：`Option<T>`
- 数组类型：`Vec<T>`

```rust
#[tool]
impl MyService {
    pub async fn process(
        &self,
        name: String,
        count: i32,
        ratio: f64,
        enabled: Option<bool>,
        tags: Vec<String>,
    ) -> Result<(), String> {
        // ...
    }
}
```

### 4. 返回值类型

支持直接返回值或 `Result<T, E>`：

```rust
#[tool]
impl MyService {
    // 直接返回
    pub async fn get_name(&self) -> String {
        "Hello".to_string()
    }

    // 带错误处理
    pub async fn parse_file(&self, path: String) -> Result<Json, String> {
        // ...
    }
}
```

### 5. 与 AI 集成

```rust
use tokitai::adapter::{AnthropicAdapter, AiAdapter, ToolDefinition};

// 转换工具定义为 AI 格式
let tools = Calculator::TOOL_DEFINITIONS
    .iter()
    .map(|t| ToolDefinition {
        name: t.name.to_string(),
        description: t.description.to_string(),
        input_schema: serde_json::from_str(t.input_schema).unwrap(),
    })
    .collect::<Vec<_>>();

// 发送给 AI
let adapter = AnthropicAdapter::new(api_key);
let response = adapter.chat(messages, None, Some(tools)).await?;

// 处理 AI 的工具调用
for tool_call in &response.tool_calls {
    let result = calc
        .call_tool(&tool_call.name, &tool_call.input)
        .await?;
    // ...
}
```

## Features

| Feature | 描述 | 依赖 |
|---------|------|------|
| `default` | 默认启用完整运行时 | 全部 |
| `runtime` | 基础运行时支持 | serde, tokio, async-trait |
| `mcp` | MCP 协议支持 | reqwest, uuid |

## 架构

```
tokitai/
├── tokitai-core/    # 核心类型定义（零依赖）
├── tokitai-macros/  # 过程宏（零依赖）
└── tokitai/         # 运行时库（可选依赖）
```

## 示例

运行示例：

```bash
# 基础用法
cargo run --example basic_usage

# 零配置示例
cargo run --example zero_config

# 文件解析示例
cargo run --example file_parser

# AI 服务器示例（需要 ANTHROPIC_API_KEY）
ANTHROPIC_API_KEY=your-key cargo run --example ai_server
```

## 许可证

MIT License
