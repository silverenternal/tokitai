# Tokitai 推广文案

## 1. Reddit r/rust 分享帖

**标题：** Showoff: tokitai - Turn any Rust method into an AI-callable tool with #[tool]

**正文：**

```markdown
Hi r/rust!

I just published **tokitai**, a zero-dependency crate that lets you expose any Rust method to AI with a single attribute macro.

## The Problem

Want to let AI call your Rust functions? Current solutions require:
- Manually writing `ToolDefinition` structs
- Hand-coding JSON parameter parsing
- Building dispatch logic with string matching
- Heavy runtime dependencies (hundreds of MBs)

## The Solution

```rust
use tokitai::tool;

pub struct Calculator;

#[tool]
impl Calculator {
    /// Add two numbers
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// Usage
let calc = Calculator;
let tools = Calculator::TOOL_DEFINITIONS;  // Compile-time generated
let result = calc.call_tool("add", &json!({"a": 10, "b": 20})).await?;
```

That's it. The macro generates:
- ✅ Tool definitions with JSON Schema
- ✅ Parameter parsing and type conversion
- ✅ Dispatch logic
- ✅ All at compile-time

## Key Features

- **Zero runtime dependencies** - Only `serde` (optional)
- **Compile-time safety** - Type errors caught at compile time, not runtime
- **No AI vendor lock-in** - Works with Claude, GPT, local models, anything
- **Lightweight** - A few KB, not hundreds of MB

## Links

- 📦 crates.io: https://crates.io/crates/tokitai
- 💻 GitHub: https://github.com/silverenternal/tokitai
- 📖 docs.rs: https://docs.rs/tokitai

## Why I Built This

I had a client request for "AI integration" - they wanted AI to call existing business logic. Existing solutions were either too heavy or locked into specific AI providers. I wanted something that just works with a `#[tool]` sticker.

## Feedback Wanted

This is my first crates.io release. I'd love feedback on:
- API design
- Documentation clarity
- Missing features
- Potential use cases

Thanks! 🦀
```

---

## 2. Rust Users Forum 官方风格帖

**标题：** [Announcement] tokitai v0.2.0 - Compile-time AI tool exposure with zero dependencies

**正文：**

```markdown
## Overview

I'm excited to announce **tokitai** v0.2.0, a new crate for exposing Rust methods to AI systems with compile-time code generation.

**Crates.io:** https://crates.io/crates/tokitai  
**GitHub:** https://github.com/silverenternal/tokitai  
**Documentation:** https://docs.rs/tokitai

## What Makes tokitai Different?

| Feature | tokitai | Other Solutions |
|---------|---------|-----------------|
| Dependencies | `serde` only | Hundreds of MB |
| Type Safety | Compile-time | Runtime |
| AI Vendor | Any | Locked-in |
| Setup | One attribute | Manual boilerplate |

## Quick Start

```toml
[dependencies]
tokitai = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

```rust
use tokitai::tool;

pub struct WeatherService;

#[tool]
impl WeatherService {
    /// Get weather forecast for a city
    pub fn get_forecast(&self, city: String, days: i32) -> Vec<String> {
        // Your business logic...
    }
}

// Get tool definitions (send to AI)
let tools = WeatherService::TOOL_DEFINITIONS;

// Handle AI tool calls
let result = service.call_tool("get_forecast", &args).await?;
```

## Architecture

tokitai consists of three crates:

- `tokitai-core` - Zero-dependency core types
- `tokitai-macros` - Procedural macro for code generation
- `tokitai` - Optional runtime with MCP protocol support

## Design Philosophy

1. **Zero runtime intrusion** - The macro does all the work
2. **No AI vendor lock-in** - Generated tools work with any AI provider
3. **Compile-time safety** - Errors caught early, not in production
4. **Minimal dependencies** - Only what you absolutely need

## Call for Feedback

This is an early release. I welcome feedback on:
- API ergonomics
- Documentation
- Feature requests
- Bug reports

Please open issues on GitHub or discuss here!

---

*Thanks to the Rust community for inspiration and support!* 🦀
```

---

## 3. Twitter/X 短推文

```
🦀 Just published tokitai v0.2.0 on crates.io!

Turn ANY Rust method into an AI-callable tool with one attribute:

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

✅ Zero runtime deps
✅ Compile-time safety
✅ No AI vendor lock-in

📦 https://crates.io/crates/tokitai
💻 https://github.com/silverenternal/tokitai

#rust #ai #programming
```

---

## 4. 中文知乎/社区文章

**标题：** 我用 Rust 写了个"魔法贴纸"库，让方法自动暴露给 AI

**正文：**

```markdown
## 背景

甲方突然说："我们要智能化，要让 AI 调用我们的业务逻辑！"

听起来很简单？实际上全是坑：
- 要手写 `ToolDefinition` 结构体
- 要解析 JSON 参数，做类型转换
- 要写路由分发逻辑
- 还要处理各种错误...

现有的方案要么依赖几百 MB，要么绑定特定 AI 供应商。

**能不能像贴贴纸一样，贴个标签就把方法暴露给 AI？**

## tokitai：一个标签搞定

```rust
use tokitai::tool;

pub struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// 使用
let calc = Calculator;
let tools = Calculator::TOOL_DEFINITIONS;  // 编译期生成
let result = calc.call_tool("add", &json!({"a": 10, "b": 20})).await?;
```

就这么简单。

## 为什么是革命性的？

### 1. Rust 生态首创

我翻遍了 crates.io，没有找到类似的库：
- ❌ 没有零配置 AI 工具暴露
- ❌ 没有编译期生成工具定义
- ❌ 没有完全不绑定供应商的轻量方案

### 2. 编译期安全

Python 的装饰器方案（如 LangChain）是运行时反射，错误在运行时才暴露。

tokitai 用过程宏在**编译期**生成所有代码：
- ✅ 类型错误编译期发现
- ✅ 参数解析自动生成
- ✅ 零运行时开销

### 3. 极致轻量

| 依赖项 | tokitai | 其他方案 |
|--------|---------|---------|
| 核心依赖 | `serde` | 几百 MB |
| 编译时间 | 几秒 | 几分钟 |
| 二进制大小 | 几 KB | 几十 MB |

### 4. 不绑定任何 AI

tokitai 只生成工具定义，你可以：
- 发给 Claude API
- 发给 OpenAI API
- 发给本地模型（Ollama、llama.cpp）
- 兼容 MCP 协议

**你说了算，不是库说了算。**

## 技术细节

tokitai 使用 `syn` 和 `quote` 在编译期：
1. 解析 impl 块中的所有 `pub` 方法
2. 从 doc comment 提取工具描述
3. 推断参数类型，生成 JSON Schema
4. 生成参数解析和分发代码

所有工作在编译期完成，运行时零开销。

## 安装使用

```toml
[dependencies]
tokitai = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## 项目链接

- 📦 crates.io: https://crates.io/crates/tokitai
- 💻 GitHub: https://github.com/silverenternal/tokitai
- 📖 文档：https://docs.rs/tokitai

## 后续计划

- [ ] 添加更多示例（对接 Claude、OpenAI）
- [ ] 完善文档
- [ ] 支持更多参数类型（嵌套结构体等）
- [ ] 社区反馈驱动开发

## 欢迎反馈

这是我的第一个 crates.io 发布，欢迎：
- Star ⭐
- Issue 🐛
- PR 💪
- 使用反馈 💬

---

*用 Rust 让 AI 集成变得简单！* 🦀
```

---

## 5. 发布顺序建议

1. **Reddit r/rust** - 技术受众，懂 Rust 价值
2. **Rust Users Forum** - 官方论坛，正式公告
3. **Twitter/X** - 短平快，扩大影响
4. **知乎/V2EX/中文社区** - 用国际社区反馈作背书

---

## 6. 关键信息

- **核心价值**：任意 Rust 方法 + `#[tool]` = AI 可调用工具
- **差异化**：编译期生成、零运行时依赖、不绑定 AI 供应商
- **目标受众**：Rust 开发者、AI 应用开发者
- **发布版本**：v0.2.0

---

*最后更新：2026 年 3 月 5 日*
*作者：silverenternal <3147264070@qq.com>*
