# Tokitai Macros

[![Crates.io](https://img.shields.io/crates/v/tokitai-macros.svg)](https://crates.io/crates/tokitai-macros)
[![Documentation](https://docs.rs/tokitai-macros/badge.svg)](https://docs.rs/tokitai-macros)
[![License](https://img.shields.io/crates/l/tokitai-macros)](../LICENSE)

## 🎯 过程宏实现

Tokitai Macros 提供了 `#[tool]` 过程宏，用于在编译期自动生成 AI 工具定义。宏本身零运行时依赖，所有代码在编译期生成。

## ✨ 核心特性

- **零运行时依赖** - 宏本身没有运行时开销
- **编译期生成** - 工具定义在编译期生成
- **类型安全** - 参数验证在编译期完成
- **自动发现** - 标记 `impl` 块后，所有 `pub` 方法自动成为工具
- **可定制** - 支持通过属性自定义工具名称和描述
- **供应商中立** - 兼容任何 AI/LLM 提供商

## 🚀 快速开始

### 添加依赖

```toml
[dependencies]
tokitai = "0.5.0"
```

**注意**：通常你不需要直接添加 `tokitai-macros` 依赖，它通过 `tokitai` crate 自动引入。

### 基本使用

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

// 使用
let calc = Calculator::default();

// 获取工具定义（编译期生成）
let tools = Calculator::tool_definitions();
println!("工具数量：{}", tools.len());

// 调用工具
let result = calc.call_tool("add", &tokitai::json!({"a": 10, "b": 20})).unwrap();
println!("结果：{}", result);  // 30
```

## 🔧 宏功能

`#[tool]` 宏自动完成以下工作：

1. **提取文档注释** 作为工具描述
2. **生成 JSON Schema** 从 Rust 类型自动映射参数
3. **创建 `tool_definitions()` 方法** 包含所有工具元数据
4. **实现 `call_tool` 分发器** 用于运行时调用
5. **生成参数解析代码** 自动验证和转换参数类型

## 📋 类型映射

Rust 类型自动映射到 JSON Schema 类型：

### 基本类型

| Rust 类型 | JSON Schema 类型 |
|-----------|------------------|
| `String`, `&str` | `string` |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | `integer` |
| `f32`, `f64` | `number` |
| `bool` | `boolean` |

### 复合类型

| Rust 类型 | JSON Schema 类型 |
|-----------|------------------|
| `Vec<T>` | `array` |
| `Option<T>` | 可选参数 |
| `HashMap<K, V>` | `object` |
| 自定义 struct | `object` |

## 🎛️ 属性语法

### 1. 标记 impl 块（推荐）

```rust
#[tool]
impl MyTools {
    // 所有 pub 方法自动注册为工具
}
```

### 2. 自定义工具名称和描述

```rust
#[tool]
impl MyTools {
    #[tool(name = "fetch_url", desc = "从 URL 获取内容")]
    pub fn fetch(&self, url: String) -> String {
        // 实现
    }
}
```

### 3. 排除方法

```rust
#[tool]
impl MyTools {
    pub fn public_tool(&self) {}

    #[tool(skip)]
    fn internal_helper(&self) {}  // 不注册为工具
}
```

### 4. 参数级别属性

```rust
#[tool]
impl MyTools {
    pub fn process(
        &self,
        #[tool(desc = "参数描述", default = "null")]
        options: Option<String>
    ) {}
}
```

## 📐 生成的代码

对于每个 `#[tool]` impl 块，宏生成：

```rust
// 1. 工具定义方法
impl Calculator {
    pub fn tool_definitions() -> &'static [ToolDefinition] {
        &[
            ToolDefinition {
                name: "add",
                description: "两个数相加",
                input_schema: "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}",
            },
            // ... 更多工具
        ]
    }
}

// 2. ToolProvider trait 实现
impl ToolProvider for Calculator {
    fn tool_definitions() -> &'static [ToolDefinition] {
        Self::tool_definitions()
    }
}

// 3. call_tool 分发器
impl Calculator {
    pub fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
        match name {
            "add" => self.__call_add(args),
            "multiply" => self.__call_multiply(args),
            _ => Err(ToolError::not_found(format!("未知工具：{}", name))),
        }
    }
}
```

## 📊 性能

| 操作 | 时间 |
|------|------|
| 宏编译时间 | < 50ms |
| 工具定义生成 | 编译期零开销 |
| `call_tool` 调用 | < 1μs |

> 基准测试环境：Rust 1.75, M1 Pro, 16GB RAM

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
| `tokitai-core` | [![crates.io](https://img.shields.io/crates/v/tokitai-core.svg)](https://crates.io/crates/tokitai-core) | 核心类型和 trait |

## 📚 文档

- **[API 文档](https://docs.rs/tokitai-macros)** - 完整的 API 参考
- **[使用指南](../docs/USAGE.md)** - 详细使用说明
- **[高级用法](../docs/ADVANCED_USAGE.md)** - 高级功能和最佳实践
- **[架构设计](../docs/ARCHITECTURE.md)** - 宏内部设计说明

---

**Happy Coding!** 🦀
