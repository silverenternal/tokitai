# Tokitai

[![Crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai)
[![Documentation](https://docs.rs/tokitai/badge.svg)](https://docs.rs/tokitai)
[![License](https://img.shields.io/crates/l/tokitai)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/silverenternal/tokitai/ci.yml)](https://github.com/silverenternal/tokitai/actions)

## 🎯 一行贴纸，让 AI 调用你的 Rust 代码

```rust
use tokitai::tool;

#[tool]  // ← 就这一行！
impl MyTools {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

**编译期生成** · **零运行时侵入** · **类型安全**

---

**编译期 AI 工具定义 · 最小运行时依赖 · 魔法贴纸式集成**

Tokitai 是一个过程宏库，只需一个 `#[tool]` 属性，即可将你的 Rust 方法自动转换为 AI 可调用的工具。所有工具定义在编译期生成，类型错误在编译时暴露。运行时仅需最小依赖（serde + serde_json），无额外开销。

## 🚀 5 分钟快速开始

### 1. 添加依赖

```toml
[dependencies]
tokitai = "0.4.0"
tokitai-mcp-server = "0.4"  # 可选：MCP 服务器脚手架
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
```

就这一行！所有必需的依赖（serde、serde_json、thiserror）都会自动包含。

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
}
```

### 3. 获取工具定义

```rust
let tools = Calculator::tool_definitions();  // v0.4.0+ 使用方法而非常量
```

### 4. 处理 AI 调用

```rust
use tokitai::json;

let calc = Calculator::default();
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}))?;
println!("{}", result);  // 30
```

### 5. 快速启动 MCP HTTP 服务器

```rust
use tokitai_mcp_server::McpServerBuilder;

#[tokio::main]
async fn main() {
    let server = McpServerBuilder::with_tool(Calculator::default())
        .with_port(8080)
        .build();

    server.run().await.unwrap();
}
```

然后从任何 MCP 客户端调用：

```python
import requests

# 获取工具列表
response = requests.get("http://127.0.0.1:8080/tools")
tools = response.json()

# 调用工具
response = requests.post(
    "http://127.0.0.1:8080/call",
    json={"name": "add", "arguments": {"a": 10, "b": 20}}
)
result = response.json()
print(result["result"])  # 30
```

### 运行示例

```bash
# 基础使用示例
cargo run --example basic_usage

# MCP 服务器示例
cargo run --example mcp_builder_demo -p tokitai-mcp-server

# 快速聊天示例（交互式）
cargo run --example quick_chat
```

## 📚 完整文档

- **[5 分钟快速开始](docs/quickstart.md)** - 详细入门教程
- **[高级用法](docs/ADVANCED_USAGE.md)** - 高级功能和最佳实践
- **[类型系统](docs/USAGE.md)** - Rust 类型到 JSON Schema 的映射
- **[AI 集成](docs/AI_INTEGRATION.md)** - 与 AI 提供商集成的指南
- **[架构说明](docs/ARCHITECTURE.md)** - 项目架构和设计
- **[API 文档](https://docs.rs/tokitai)** - 完整的 API 参考

## ✨ 核心特性

| 特性 | 说明 |
|------|------|
| **最小依赖侵入** | 用户只需添加 `tokitai = "0.4.0`，运行时仅需 serde + serde_json |
| **编译期生成** | 工具定义在编译期生成，类型错误早发现 |
| **单一属性** | 只需 `#[tool]`，无需多个标签 |
| **类型安全** | Rust 类型自动映射到 JSON Schema |
| **供应商中立** | 支持任何 AI/LLM 提供商 |

## 📋 类型映射

| Rust 类型 | JSON Schema |
|-----------|-------------|
| `String`, `&str` | `string` |
| `i32`, `i64`, `u32` 等 | `integer` |
| `f32`, `f64` | `number` |
| `bool` | `boolean` |
| `Vec<T>` | `array` |
| `Option<T>` | 可选 `T` |
| 自定义 struct | `object` |

## 🔧 常用属性

```rust
#[tool]
impl MyTools {
    /// 自定义名称
    #[tool(name = "custom_name")]
    pub fn my_func(&self) {}

    /// 自定义描述
    #[tool(desc = "自定义描述")]
    pub fn another_func(&self) {}

    /// 参数级别属性
    pub fn process(
        &self,
        #[tool(desc = "参数描述", default = "null")] 
        options: Option<String>
    ) {}
}
```

完整属性列表见 [高级用法](docs/ADVANCED_USAGE.md)。

## ⚡ 性能

| 操作 | 时间 |
|------|------|
| 宏编译时间 | < 50ms |
| 工具定义生成 | 编译期零开销 |
| `call_tool` 调用 | < 1μs |

> 基准测试环境：Rust 1.75, M1 Pro, 16GB RAM
>
> 运行基准测试：`cargo bench --bench macro_bench`

## 📦 项目结构

Tokitai 由三个 crate 组成：

| Crate | 说明 |
|-------|------|
| `tokitai` | 主 crate，包含运行时支持 |
| `tokitai-core` | 核心类型和 trait（零依赖） |
| `tokitai-macros` | 过程宏实现 |

**99% 的用户只需要：**
```toml
[dependencies]
tokitai = "0.4.0"
```

## ⚙️ 要求

- **Rust 版本**: 1.80+
- **Edition**: 2021

## 📄 许可证

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE))
- MIT License ([LICENSE-MIT](LICENSE))

at your option.

## 🤝 贡献

除非你明确声明其他许可，否则你为本 crate 提交的所有贡献都将按上述两种方式之一授权，无需额外条款或条件。

## 📝 示例

更多示例见 [examples 目录](examples/)：

- `basic_usage.rs` - 基础使用示例
- `advanced_types.rs` - 高级类型和功能完整演示
- `mcp_server_demo.rs` - MCP 服务器示例
- `mcp_http_server.rs` - HTTP 服务器示例
- `ollama_integration.rs` - Ollama AI 集成

## 🔒 API 稳定承诺

Tokitai 遵循 [语义化版本](https://semver.org/)，详细的 API 稳定政策见 [API 稳定承诺](docs/API_STABILITY.md)。

**当前状态**: v0.4.x 系列 - 核心 API 已稳定，v1.0.0 计划中。

---

**Happy Coding!** 🦀
