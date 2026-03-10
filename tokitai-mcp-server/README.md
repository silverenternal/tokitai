# Tokitai MCP Server

[![Crates.io](https://img.shields.io/crates/v/tokitai-mcp-server.svg)](https://crates.io/crates/tokitai-mcp-server)
[![Documentation](https://docs.rs/tokitai-mcp-server/badge.svg)](https://docs.rs/tokitai-mcp-server)
[![License](https://img.shields.io/crates/l/tokitai-mcp-server)](../LICENSE)

## 🎯 MCP 服务器脚手架

Tokitai MCP Server 提供了基于 MCP (Model Context Protocol) 协议的服务器实现，让你能够快速构建 AI 可调用的工具服务器。

## ✨ 核心特性

- **零运行时开销** - 工具定义在编译期生成
- **类型安全** - Rust 类型系统确保 AI 参数与函数签名匹配
- **MCP 兼容** - 完整支持 MCP 协议规范
- **易于使用** - 几行代码即可启动服务器
- **HTTP 支持** - 可选的 HTTP 服务器，支持 RESTful API

## 🚀 快速开始

### 1. 添加依赖

```toml
[dependencies]
tokitai = "0.4.0"
tokitai-mcp-server = "0.4.0"
tokio = { version = "1", features = ["full"] }
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
}
```

### 3. 创建并运行服务器

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

### 4. 从 AI 客户端调用

```python
# Python MCP 客户端示例
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

## 📚 核心类型

| 类型 | 说明 |
|------|------|
| [`McpServer`] | MCP 服务器核心类型 |
| [`McpServerBuilder`] | 流式构建器，用于创建服务器 |
| [`MultiToolProvider`] | 多工具提供者，支持注册多个工具集 |

## 🏗️ 使用多工具提供者

```rust
use tokitai::tool;
use tokitai_mcp_server::MultiToolProvider;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

#[tool]
struct Greeter;

#[tool]
impl Greeter {
    pub fn greet(&self, name: String) -> String {
        format!("Hello, {}!", name)
    }
}

// 组合多个工具集
let mut provider = MultiToolProvider::new();
provider.add(Calculator::default());
provider.add(Greeter::default());
```

## 📋 API 端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/tools` | GET | 获取所有可用工具的定义 |
| `/call` | POST | 调用指定工具 |
| `/health` | GET | 健康检查端点 |

## ⚙️ 配置选项

使用 `McpServerBuilder` 配置服务器：

```rust
let server = McpServerBuilder::with_tool(my_tool)
    .with_port(8080)           // 设置监听端口
    .with_host("0.0.0.0")      // 设置监听地址
    .build();
```

## 🎛️ Features

| Feature | 说明 |
|---------|------|
| `default` | 默认配置 |

## ⚙️ 要求

- **Rust 版本**: 1.80+
- **Edition**: 2021

## 📄 许可证

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE](../LICENSE))
- MIT License ([LICENSE](../LICENSE))

at your option.

## 📦 相关 Crate

| Crate | Crates.io | 说明 |
|-------|-----------|------|
| `tokitai` | [![crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai) | 主 crate，包含运行时支持 |
| `tokitai-core` | [![crates.io](https://img.shields.io/crates/v/tokitai-core.svg)](https://crates.io/crates/tokitai-core) | 核心类型和 trait |
| `tokitai-macros` | [![crates.io](https://img.shields.io/crates/v/tokitai-macros.svg)](https://crates.io/crates/tokitai-macros) | 过程宏实现 |

## 📚 文档

- **[API 文档](https://docs.rs/tokitai-mcp-server)** - 完整的 API 参考
- **[快速开始](../docs/quickstart.md)** - 5 分钟入门教程
- **[MCP 架构](../docs/MCP_ARCHITECTURE.md)** - MCP 协议设计说明

---

**Happy Coding!** 🦀
