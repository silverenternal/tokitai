//! Tokitai 过程宏 - 零配置 AI 工具暴露
//!
//! # 使用示例
//!
//! ## 方案 1：单一宏，自动发现
//!
//! ```rust,ignore
//! use tokitai_macros::tool;
//!
//! pub struct Calculator;
//!
//! #[tool]
//! impl Calculator {
//!     /// 两个数相加
//!     pub async fn add(&self, a: i32, b: i32) -> i32 {
//!         a + b
//!     }
//! }
//! ```
//!
//! ## 方案 2：带自定义属性
//!
//! ```rust,ignore
//! #[tool]
//! impl Calculator {
//!     #[tool(name = "add_numbers", desc = "将两个数字相加")]
//!     pub async fn add(&self, a: i32, b: i32) -> i32 {
//!         a + b
//!     }
//! }
//! ```

mod tool;

use proc_macro::TokenStream;

/// `#[tool]` 属性宏
///
/// 标记 impl 块为工具提供者，或标记单个方法为工具。
///
/// # 用法
///
/// ## 1. 标记 impl 块（推荐）
///
/// 在 impl 块上使用 `#[tool]`，会自动将所有 `pub` 方法注册为工具：
///
/// ```rust,ignore
/// #[tool]
/// impl Calculator {
///     /// 两个数相加
///     pub async fn add(&self, a: i32, b: i32) -> i32 {
///         a + b
///     }
/// }
/// ```
///
/// ## 2. 标记单个方法
///
/// 在方法上使用 `#[tool(...)]` 可自定义工具属性：
///
/// ```rust,ignore
/// #[tool]
/// impl Calculator {
///     #[tool(name = "add_numbers", desc = "将两个数字相加")]
///     pub async fn add(&self, a: i32, b: i32) -> i32 {
///         a + b
///     }
///
///     /// 这个方法不会被注册为工具
///     fn helper(&self) {}
/// }
/// ```
///
/// # 生成的代码
///
/// 宏会生成：
///
/// 1. `const TOOL_DEFINITIONS: &'static [ToolDefinition]` - 编译期工具定义
/// 2. `fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError>` - 工具调用分发
/// 3. 每个工具的包装函数，用于 JSON 参数解析
///
/// # 特性
///
/// - ✅ 零运行时依赖（宏本身）
/// - ✅ 编译期生成工具定义
/// - ✅ 自动从 doc comment 提取描述
/// - ✅ 支持自定义工具名称和描述
/// - ✅ 类型安全的参数解析
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool::tool(attr, item)
}
