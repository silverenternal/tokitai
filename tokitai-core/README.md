# Tokitai Core

[![Crates.io](https://img.shields.io/crates/v/tokitai-core.svg)](https://crates.io/crates/tokitai-core)
[![Documentation](https://docs.rs/tokitai-core/badge.svg)](https://docs.rs/tokitai-core)
[![License](https://img.shields.io/crates/l/tokitai-core)](../LICENSE)

## 🎯 核心类型定义

Tokitai Core 提供了 Tokitai AI 工具集成系统的基础类型和 trait。所有工具信息在编译期生成，确保零运行时开销和最大的类型安全。

## ✨ 核心特性

- **零运行时依赖** - 核心类型依赖最小化
- **类型安全** - 编译期工具定义防止运行时错误
- **Serde 集成** - 通过 `serde` 特性提供可选的序列化支持

## 📦 核心类型

| 类型 | 说明 |
|------|------|
| [`ToolDefinition`] | 工具定义，包含名称、描述和输入 schema |
| [`ToolParameter`] | 工具的参数定义 |
| [`ParamType`] | JSON Schema 类型枚举 |
| [`ToolError`] | 工具调用错误类型 |
| [`ToolErrorKind`] | 错误分类枚举 |
| [`ToolProvider`] | 工具提供者 trait（由 `#[tool]` 宏自动实现） |

## 🚀 快速开始

### 添加依赖

```toml
[dependencies]
tokitai-core = "0.5"
```

### 基本使用

```rust
use tokitai_core::ToolDefinition;

// 创建工具定义
let tool = ToolDefinition::new(
    "add",
    "两个数相加",
    r#"{"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}},"required":["a","b"]}"#
);

assert_eq!(tool.name, "add");
assert_eq!(tool.description, "两个数相加");

// 启用 serde 特性时，可转换为 JSON
#[cfg(feature = "serde")]
{
    let json = tool.to_json().unwrap();
    println!("{}", json);
}
```

## 📋 类型映射

[`ParamType`] 枚举将 Rust 类型映射到 JSON Schema 类型：

| Rust 类型 | JSON Schema 类型 | `ParamType` 变体 |
|-----------|------------------|------------------|
| `String`, `&str` | `string` | `ParamType::String` |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | `integer` | `ParamType::Integer` |
| `f32`, `f64` | `number` | `ParamType::Number` |
| `bool` | `boolean` | `ParamType::Boolean` |
| `Vec<T>` | `array` | `ParamType::Array` |
| 自定义 struct | `object` | `ParamType::Object` |

```rust
use tokitai_core::ParamType;

assert_eq!(ParamType::from_rust_type("String"), Some(ParamType::String));
assert_eq!(ParamType::from_rust_type("i32"), Some(ParamType::Integer));
assert_eq!(ParamType::from_rust_type("f64"), Some(ParamType::Number));
assert_eq!(ParamType::from_rust_type("bool"), Some(ParamType::Boolean));
assert_eq!(ParamType::from_rust_type("Vec<i32>"), Some(ParamType::Array));
```

## 🔧 错误处理

[`ToolError`] 类型为工具调用提供结构化错误处理：

```rust
use tokitai_core::{ToolError, ToolErrorKind};

// 创建不同类型的错误
let validation_err = ToolError::validation_error("缺少必需参数 'city'");
assert_eq!(validation_err.kind, ToolErrorKind::ValidationError);

let not_found_err = ToolError::not_found("工具 'unknown_tool' 未找到");
assert_eq!(not_found_err.kind, ToolErrorKind::NotFound);

let internal_err = ToolError::internal_error("连接超时");
assert_eq!(internal_err.kind, ToolErrorKind::InternalError);
```

### 错误类型

| 变体 | 值 | 说明 |
|------|-----|------|
| `ToolErrorKind::ValidationError` | 0 | 参数验证失败 |
| `ToolErrorKind::NotFound` | 1 | 工具未找到 |
| `ToolErrorKind::InternalError` | 2 | 内部错误 |
| `ToolErrorKind::TypeError` | 3 | 类型错误 |

## 🏗️ ToolProvider Trait

[`ToolProvider`] trait 由 `#[tool]` 宏自动实现：

```rust
use tokitai_core::ToolProvider;

// 在你的类型上使用 #[tool] 宏后：
// struct Calculator;
// #[tool] impl Calculator { ... }

// 获取所有工具定义
let tools = Calculator::tool_definitions();

// 获取工具数量
let count = Calculator::tool_count();

// 查找特定工具
let tool = Calculator::find_tool("add");
```

## 📐 JSON Schema 宏

`json_schema!` 宏帮助在编译期生成 JSON Schema 字符串：

```rust,ignore
use tokitai_core::json_schema;

const SCHEMA: &str = json_schema!({
    "city": {
        type: String,
        description: "城市名称",
        required: true,
    }
});
```

## 🎛️ Features

| Feature | 说明 |
|---------|------|
| `serde` (默认启用) | 启用 serde 序列化和 JSON 支持 |

## ⚙️ 要求

- **Rust 版本**: 1.80+
- **Edition**: 2021

## 📄 许可证

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE](../LICENSE))
- MIT License ([LICENSE](../LICENSE))

at your option.

## 🤝 贡献

除非你明确声明其他许可，否则你为本 crate 提交的所有贡献都将按上述两种方式之一授权，无需额外条款或条件。

## 📦 相关 Crate

| Crate | Crates.io | 说明 |
|-------|-----------|------|
| `tokitai` | [![crates.io](https://img.shields.io/crates/v/tokitai.svg)](https://crates.io/crates/tokitai) | 主 crate，包含运行时支持 |
| `tokitai-macros` | [![crates.io](https://img.shields.io/crates/v/tokitai-macros.svg)](https://crates.io/crates/tokitai-macros) | 过程宏实现 |

## 📚 文档

- **[API 文档](https://docs.rs/tokitai-core)** - 完整的 API 参考
- **[使用指南](../docs/USAGE.md)** - 详细使用说明
- **[架构设计](../docs/ARCHITECTURE.md)** - 内部设计说明

---

**Happy Coding!** 🦀
