# Tokitai

**编译期 AI 工具定义 · 零运行时侵入 · 魔法贴纸式集成**

只需一个 `#[tool]` 标签，让你的 Rust 方法立即可以被 AI 调用。

## 快速开始

```rust
use tokitai::tool;

pub struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 解析 DXF 文件
    pub fn parse_dxf(&self, path: String) -> Result<DxfData, ParseError> {
        // 你的业务逻辑...
    }
}

// 使用
let calc = Calculator;

// 获取工具列表（发送给 AI）
let tools = Calculator::TOOL_DEFINITIONS;

// 调用工具（接收 AI 的请求）
let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20}))?;
```

## 特性

- 🏷️ **魔法贴纸** - 只需 `#[tool]`，无需学习复杂 API
- 🔒 **编译期生成** - 工具定义在编译期生成，类型错误编译时暴露
- 🪶 **零运行时依赖** - `default-features = false` 仅依赖 `serde`
- 🔌 **不绑定 AI 供应商** - 生成的工具定义可发给任何 AI（Claude、GPT 等）
- 📦 **MCP 兼容** - 可选 MCP 协议支持

## 安装

```toml
[dependencies]
tokitai = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 最小化依赖

```toml
[dependencies]
tokitai = { version = "0.2", default-features = false }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## 使用场景

### 1. 暴露现有方法给 AI

```rust
use tokitai::tool;

pub struct DxfParser { /* ... */ }

#[tool]
impl DxfParser {
    /// 解析 DXF 文件，提取几何信息
    pub fn parse(&self, file_path: String) -> Result<ParsedData, ParseError> {
        // 原有业务逻辑...
    }
}
```

### 2. 与 AI SDK 配合

```rust
// 1. 获取工具定义，发送给 AI
let tools = Calculator::TOOL_DEFINITIONS;
let ai_request = build_ai_request(tools, user_message);

// 2. AI 返回工具调用
let ai_response = call_ai_api(ai_request).await?;

// 3. 执行工具调用
let result = calc.call_tool(&ai_response.tool_name, &ai_response.args)?;

// 4. 返回结果给 AI
let final_response = build_ai_response(result);
```

### 3. MCP 协议支持

```toml
tokitai = { version = "0.2", features = ["mcp"] }
```

```rust
use tokitai::{tool, mcp};

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 { ... }
}

// 转换为 MCP 格式
let mcp_tools = mcp::to_mcp_tools(Calculator::TOOL_DEFINITIONS);
```

## 宏生成内容

`#[tool]` 宏自动为 impl 块生成：

1. `const TOOL_DEFINITIONS: &'static [ToolDefinition]` - 编译期工具定义数组
2. `fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError>` - 工具调用分发
3. 每个工具的包装函数，用于 JSON 参数解析

## 支持的参数类型

| Rust 类型 | JSON Schema 类型 |
|-----------|-----------------|
| `String`, `&str` | `string` |
| `i32`, `i64`, `u32` 等 | `integer` |
| `f32`, `f64` | `number` |
| `bool` | `boolean` |
| `Vec<T>` | `array` |
| `Option<T>` | 可选参数 |
| 其他 `serde::Deserialize` 类型 | `object` |

## 自定义工具属性

```rust
#[tool]
impl Calculator {
    #[tool(name = "add_numbers", desc = "将两个数字相加")]
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

## 项目结构

```
tokitai/
├── tokitai-core/     # 核心类型定义（零依赖）
├── tokitai-macros/   # 过程宏（编译期代码生成）
└── tokitai/          # 运行时库（可选）
```

## 许可证

MIT

## 作者

silverenternal <3147264070@qq.com>

## 贡献

欢迎提交 Issue 和 PR！

## 链接

- GitHub: https://github.com/silverenternal/tokitai
- 文档：https://docs.rs/tokitai
