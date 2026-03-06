# Tokitai 使用指南

## 目录

1. [快速开始](#快速开始)
2. [安装配置](#安装配置)
3. [基础用法](#基础用法)
4. [高级特性](#高级特性)
5. [最佳实践](#最佳实践)

---

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

    /// 两个数相乘
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
```

### 3. 使用工具

```rust
let calc = Calculator;

// 获取工具定义（发送给 AI）
let tools = Calculator::TOOL_DEFINITIONS;

// 调用工具（接收 AI 的请求）
let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20})).await?;
```

---

## 安装配置

### 完整安装（推荐）

```toml
[dependencies]
tokitai = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 最小化安装

```toml
[dependencies]
tokitai = { version = "0.2", default-features = false }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 启用 MCP 支持

```toml
[dependencies]
tokitai = { version = "0.2", features = ["mcp"] }
```

### Features 说明

| Feature | 描述 | 依赖 |
|---------|------|------|
| `default` | 启用完整运行时 | `serde`, `serde_json`, `thiserror` |
| `runtime` | 基础运行时支持 | `serde`, `serde_json`, `thiserror` |
| `mcp` | MCP 协议支持 | 需要 `runtime` |

---

## 基础用法

### 自动注册方法

在 `impl` 块上使用 `#[tool]` 宏，所有 `pub` 方法会自动注册为工具：

```rust
use tokitai::tool;

pub struct WeatherService;

#[tool]
impl WeatherService {
    /// 获取指定城市的天气
    pub fn get_weather(&self, city: String) -> String {
        format!("{} 的天气：晴朗", city)
    }

    /// 获取多日预报
    pub fn get_forecast(&self, city: String, days: i32) -> Vec<String> {
        (0..days).map(|i| format!("第 {} 天：晴朗", i + 1)).collect()
    }
}
```

### 自定义工具属性

使用 `#[tool(name = "...", desc = "...")]` 自定义工具名称和描述：

```rust
#[tool]
impl WeatherService {
    #[tool(name = "fetch_weather", desc = "从外部 API 获取天气数据")]
    pub fn get_weather(&self, city: String) -> String {
        // 调用外部 API...
    }
}
```

### 支持的方法签名

#### 同步方法

```rust
#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

#### 异步方法

```rust
#[tool]
impl Database {
    pub async fn query(&self, sql: String) -> Result<Vec<Row>, DbError> {
        // 异步数据库查询...
    }
}
```

#### 返回 Result

```rust
#[tool]
impl Parser {
    pub fn parse(&self, input: String) -> Result<Data, ParseError> {
        // 可能失败的操作...
    }
}
```

---

## 高级特性

### 支持的参数类型

| Rust 类型 | JSON Schema 类型 | 示例 |
|-----------|-----------------|------|
| `String`, `&str` | `string` | `"hello"` |
| `i8`..=`i128`, `u8`..=`u128` | `integer` | `42` |
| `f32`, `f64` | `number` | `3.14` |
| `bool` | `boolean` | `true` |
| `Vec<T>` | `array` | `[1, 2, 3]` |
| `Option<T>` | 可选参数 | `null` 或值 |
| 自定义类型 | `object` | `{"field": "value"}` |

### 可选参数

```rust
#[tool]
impl SearchEngine {
    /// 搜索文档
    pub fn search(
        &self,
        query: String,
        limit: Option<i32>,  // 可选参数
        offset: Option<i32>, // 可选参数
    ) -> Vec<Document> {
        // ...
    }
}
```

### 自定义类型参数

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

#[tool]
impl MapService {
    /// 获取指定地点的天气
    pub fn get_weather_at(&self, location: Location) -> String {
        format!("位置 ({}, {}) 的天气", location.latitude, location.longitude)
    }
}
```

### 获取工具定义

```rust
use tokitai::ToolProvider;

// 获取所有工具定义
let tools = Calculator::TOOL_DEFINITIONS;

// 获取工具数量
let count = Calculator::tool_count();

// 查找特定工具
if let Some(tool) = Calculator::find_tool("add") {
    println!("找到工具：{}", tool.name);
}
```

### 工具调用

```rust
// 同步调用
let result = calc.call_tool("add", &json!({"a": 10, "b": 20})).await?;

// 处理错误
match calc.call_tool("divide", &json!({"a": 10, "b": 0})).await {
    Ok(result) => println!("结果：{}", result),
    Err(e) => eprintln!("错误：{:?}", e),
}
```

---

## 最佳实践

### 1. 工具命名

- 使用清晰的动词 + 名词格式：`get_weather`, `create_user`, `delete_file`
- 避免使用缩写，除非是通用缩写
- 保持命名一致性

### 2. 文档注释

为每个工具方法添加清晰的文档注释，这些会被自动提取为工具描述：

```rust
#[tool]
impl Calculator {
    /// 计算两个整数的和，返回结果
    /// 
    /// # 参数
    /// - `a`: 第一个整数
    /// - `b`: 第二个整数
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

### 3. 错误处理

使用 `Result` 类型返回可能的错误：

```rust
#[tool]
impl FileService {
    pub fn read_file(&self, path: String) -> Result<String, String> {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("读取文件失败：{}", e))
    }
}
```

### 4. 工具分组

将相关工具组织在同一个 struct 中：

```rust
// 用户管理工具
pub struct UserService { /* ... */ }

#[tool]
impl UserService {
    pub fn create_user(&self, name: String) -> User { /* ... */ }
    pub fn get_user(&self, id: i32) -> Option<User> { /* ... */ }
    pub fn delete_user(&self, id: i32) -> bool { /* ... */ }
}

// 订单管理工具
pub struct OrderService { /* ... */ }

#[tool]
impl OrderService {
    pub fn create_order(&self, user_id: i32) -> Order { /* ... */ }
    pub fn get_orders(&self, user_id: i32) -> Vec<Order> { /* ... */ }
}
```

### 5. 性能考虑

- 工具方法应尽量轻量，避免长时间阻塞
- 对于耗时操作，使用异步方法
- 考虑使用缓存减少重复计算

---

## 示例代码

更多示例请查看：

- [`examples/basic_usage.rs`](../examples/basic_usage.rs) - 基础使用示例
- [`examples/ollama_integration.rs`](../examples/ollama_integration.rs) - Ollama AI 集成
- [`examples/multi_tool_chat.rs`](../examples/multi_tool_chat.rs) - 多工具协作

---

## 常见问题

### Q: 宏生成的代码在哪里？

A: 使用 `cargo expand` 可以查看宏展开后的代码：

```bash
cargo install cargo-expand
cargo expand --example basic_usage
```

### Q: 如何调试工具调用？

A: 启用日志记录：

```rust
env_logger::init();
```

### Q: 支持哪些 AI 平台？

A: Tokitai 生成的工具定义是通用的，可以与任何支持工具调用的 AI 平台配合使用：
- Ollama
- Claude
- GPT-4
- 其他支持 Function Calling 的平台
