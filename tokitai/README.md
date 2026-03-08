# Tokitai

[![Crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai)
[![Documentation](https://docs.rs/tokitai/badge.svg)](https://docs.rs/tokitai)
[![License](https://img.shields.io/crates/l/tokitai)](../LICENSE)
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

**编译期 AI 工具定义 · 零运行时侵入 · 魔法贴纸式集成**

Tokitai 是一个零运行时依赖的过程宏库，只需一个 `#[tool]` 属性，即可将你的 Rust 方法自动转换为 AI 可调用的工具。所有工具定义在编译期生成，类型错误在编译时暴露。

## 🚀 5 分钟快速开始

### 1. 添加依赖

```toml
[dependencies]
tokitai = "0.3.3"
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
let tools = Calculator::TOOL_DEFINITIONS;
```

### 4. 处理 AI 调用

```rust
use tokitai::json;

let calc = Calculator;
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}))?;
println!("{}", result);  // 30
```

## ✨ 核心特性

| 特性 | 说明 |
|------|------|
| **零依赖侵入** | 用户只需添加 `tokitai = "0.3.3"` |
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

## 📚 文档

- **[5 分钟快速开始](docs/quickstart.md)** - 详细入门教程
- **[高级用法](docs/ADVANCED_USAGE.md)** - 高级功能和最佳实践
- **[类型系统](docs/USAGE.md)** - Rust 类型到 JSON Schema 的映射
- **[AI 集成](docs/AI_INTEGRATION.md)** - 与 AI 提供商集成的指南
- **[架构说明](docs/ARCHITECTURE.md)** - 项目架构和设计
- **[API 文档](https://docs.rs/tokitai)** - 完整的 API 参考

## ⚙️ 要求

- **Rust 版本**: 1.70+
- **Edition**: 2021

## 📄 许可证

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE](../LICENSE))
- MIT License ([LICENSE](../LICENSE))

at your option.

## 🤝 贡献

除非你明确声明其他许可，否则你为本 crate 提交的所有贡献都将按上述两种方式之一授权，无需额外条款或条件。

## 📦 子 crate

Tokitai 由三个 crate 组成：

| Crate | Crates.io | 说明 |
|-------|-----------|------|
| `tokitai` | [![crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai) | 主 crate，包含运行时支持 |
| `tokitai-core` | [![crates.io](https://img.shields.io/crates/v/tokitai-core.svg)](https://crates.io/crates/tokitai-core) | 核心类型和 trait（零依赖） |
| `tokitai-macros` | [![crates.io](https://img.shields.io/crates/v/tokitai-macros.svg)](https://crates.io/crates/tokitai-macros) | 过程宏实现 |

**99% 的用户只需要：**
```toml
[dependencies]
tokitai = "0.3.3"
```

## 📝 示例

更多示例见 [examples 目录](../examples/)：

- `basic_usage.rs` - 基础使用示例
- `quick_chat.rs` - 交互式数学计算器
- `version_management.rs` - 版本管理属性演示
- `ollama_integration.rs` - Ollama AI 集成

---

**Happy Coding!** 🦀
