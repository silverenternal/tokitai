//! Tokitai - AI 工具集成系统
//!
//! # 🎯 编译期工具定义，零运行时侵入
//!
//! 只需在 impl 块上贴 `#[tool]`，宏自动生成所有工具定义和调用逻辑。
//!
//! ## 快速开始
//!
//! ```rust,ignore
//! use tokitai::tool;
//!
//! pub struct Calculator;
//!
//! #[tool]
//! impl Calculator {
//!     /// 两个数相加
//!     pub fn add(&self, a: i32, b: i32) -> i32 {
//!         a + b
//!     }
//!
//!     /// 两个数相乘
//!     pub fn multiply(&self, a: i32, b: i32) -> i32 {
//!         a * b
//!     }
//! }
//!
//! // 使用
//! let calc = Calculator;
//!
//! // 获取工具列表（编译期生成）
//! let tools = Calculator::TOOL_DEFINITIONS;
//! println!("工具数量：{}", tools.len());
//!
//! // 调用工具
//! let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20})).unwrap();
//! println!("结果：{}", result);  // 30
//! ```
//!
//! ## 特性
//!
//! - ✅ **零运行时侵入** - 宏本身零依赖，不强制绑定任何运行时
//! - ✅ **编译期类型安全** - 工具定义在编译期生成，参数类型错误编译时暴露
//! - ✅ **单一宏** - 只需 `#[tool]`，无需同时贴多个标签
//! - ✅ **可选运行时** - 通过 features 控制依赖，支持无异步环境
//!
//! ## Features
//!
//! | Feature | 描述 |
//! |---------|------|
//! | `default` | 启用完整运行时 |
//! | `runtime` | 基础运行时支持（异步、错误处理） |
//! | `mcp` | MCP 协议支持 |
//!
//! ## 最小化依赖（仅编译期）
//!
//! ```toml
//! [dependencies]
//! tokitai = { version = "0.2", default-features = false }
//! serde = { version = "1.0", features = ["derive"] }
//! serde_json = "1.0"
//! ```

// 核心类型重新导出（总是可用）
pub use tokitai_core::{ToolDefinition, ToolError, ToolErrorKind, ParamType, ToolProvider};

// 运行时模块（可选）
#[cfg(feature = "runtime")]
pub mod error;

#[cfg(feature = "mcp")]
pub mod mcp;

// 条件导出运行时类型
#[cfg(feature = "runtime")]
pub use error::AiToolError;

#[cfg(feature = "mcp")]
pub use mcp::*;

// 重新导出宏
pub use tokitai_macros::tool;

/// 库版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
