# ADR 001: ToolCallerDyn Trait (现为 ToolCaller)

**状态**: 已接受  
**日期**: 2026 年 3 月  
**优先级**: P1

## 背景

在实现 MCP 服务器时，我们需要在运行时调用工具（处理来自 AI 的动态请求）。这要求我们有一个运行时工具调用的接口。

## 决策

添加 `ToolCaller` trait（最初命名为 `ToolCallerDyn`）到 `tokitai-core`：

```rust
pub trait ToolCaller {
    fn call_tool(&self, name: &str, args: &serde_json::Value) -> Result<serde_json::Value, ToolError>;
}
```

## 原因

1. **编译期与运行期的分离**: `#[tool]` 宏在编译期生成所有工具定义和调用逻辑，但 MCP 服务器需要在运行时接收 AI 的请求并动态调用工具。

2. **类型安全**: 通过 trait 约束，确保只有实现了 `ToolCaller` 的类型才能被用于 MCP 服务器。

3. **灵活性**: 允许用户自定义工具调用逻辑，而不仅限于宏生成的代码。

## 宏生成

`#[tool]` 宏现在自动为所有标记的类型实现 `ToolCaller`：

```rust
#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// 宏自动生成：
// impl ToolCaller for Calculator { ... }
```

## 使用示例

```rust
use tokitai::{ToolProvider, ToolCaller};

let calc = Calculator::default();

// 编译期生成的工具定义
let tools = Calculator::tool_definitions();

// 运行时工具调用（来自 AI 的请求）
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}))?;
```

## 后果

### 正面
- ✅ 类型安全的运行时工具调用
- ✅ 宏自动生成，用户无需手动实现
- ✅ 支持 MCP 服务器的动态调用需求

### 负面
- ❌ 增加了一个 trait，略微增加 API 复杂度
- ❌ 需要文档说明 `ToolProvider`（编译期）和 `ToolCaller`（运行时）的区别

## 替代方案

1. **不使用 trait，直接使用具体类型**: 会失去灵活性，无法支持自定义工具提供者。

2. **使用 `Any::downcast_ref` 做类型判断**: 这是我们在 `get_tools_from_provider_runtime` 中使用的方法，用于处理 `MultiToolProvider` 的特殊情况。

## 参考

- MCP 服务器实现：`tokitai-mcp-server/src/server.rs`
- `ToolCaller` trait 定义：`tokitai-core/src/lib.rs`
- 宏实现：`tokitai-macros/src/tool.rs`
