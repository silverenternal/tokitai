//! Doc-tests for tokitai-macros crate
//!
//! 运行测试：cargo test -p tokitai-macros --doc

#![cfg(feature = "serde")]

/// 基本工具定义示例
///
/// ```rust,ignore
/// use tokitai::tool;
/// use tokitai::ToolProvider;
///
/// #[derive(Default)]
/// pub struct Calculator;
///
/// #[tool]
/// impl Calculator {
///     /// 两个数相加
///     pub fn add(&self, a: i32, b: i32) -> i32 {
///         a + b
///     }
/// }
///
/// // 获取工具定义
/// let tools = Calculator::tool_definitions();
/// assert_eq!(tools.len(), 1);
/// ```
#[test]
fn test_basic_tool_doc() {
    // 这个测试在 doc 中展示
}

/// 可选参数示例
///
/// ```rust,ignore
/// use tokitai::tool;
///
/// #[derive(Default)]
/// pub struct Greeter;
///
/// #[tool]
/// impl Greeter {
///     /// 打招呼，可选的语言参数
///     pub fn greet(&self, name: String, language: Option<String>) -> String {
///         match language.as_deref() {
///             Some("zh") => format!("你好，{}！", name),
///             _ => format!("Hello, {}!", name),
///         }
///     }
/// }
/// ```
#[test]
fn test_option_param_doc() {
    // 这个测试在 doc 中展示
}

/// Result 返回类型示例
///
/// ```rust,ignore
/// use tokitai::tool;
/// use thiserror::Error;
///
/// #[derive(Error, Debug)]
/// pub enum MathError {
///     #[error("除数不能为零")]
///     DivisionByZero,
/// }
///
/// #[derive(Default)]
/// pub struct MathService;
///
/// #[tool]
/// impl MathService {
///     /// 两个数相除
///     pub fn divide(&self, a: f64, b: f64) -> Result<f64, MathError> {
///         if b == 0.0 {
///             Err(MathError::DivisionByZero)
///         } else {
///             Ok(a / b)
///         }
///     }
/// }
/// ```
#[test]
fn test_result_return_doc() {
    // 这个测试在 doc 中展示
}

/// 自定义工具属性示例
///
/// ```rust,ignore
/// use tokitai::tool;
///
/// #[derive(Default)]
/// pub struct DataProcessor;
///
/// #[tool]
/// impl DataProcessor {
///     #[tool(name = "process_data", desc = "处理数据并返回结果")]
///     pub fn process(&self, input: String) -> String {
///         format!("Processed: {}", input)
///     }
/// }
/// ```
#[test]
fn test_custom_attrs_doc() {
    // 这个测试在 doc 中展示
}

/// 参数验证属性示例
///
/// ```rust,ignore
/// use tokitai::tool;
/// use tokitai::param_tool;
///
/// #[derive(Default)]
/// pub struct UserCreator;
///
/// #[tool]
/// impl UserCreator {
///     pub fn create_user(
///         &self,
///         #[param_tool(desc = "用户名", min_length = 3)]
///         username: String,
///         #[param_tool(desc = "邮箱", pattern = "@")]
///         email: String,
///     ) -> String {
///         format!("User: {}", username)
///     }
/// }
/// ```
#[test]
fn test_param_validation_doc() {
    // 这个测试在 doc 中展示
}

/// 配置宏示例
///
/// ```rust,ignore
/// use tokitai::tool;
/// use tokitai::config;
///
/// #[derive(Default)]
/// pub struct MyService;
///
/// #[tool]
/// impl MyService {
///     pub fn get_user(&self, id: i32) -> String {
///         format!("User {}", id)
///     }
/// }
///
/// config! {
///     MyService {
///         get_user: {
///             desc: "获取用户信息",
///             params: {
///                 id: { desc: "用户 ID" }
///             }
///         }
///     }
/// }
/// ```
#[test]
fn test_config_macro_doc() {
    // 这个测试在 doc 中展示
}

/// 异步方法示例
///
/// ```rust,ignore
/// use tokitai::tool;
///
/// #[derive(Default)]
/// pub struct AsyncService;
///
/// #[tool]
/// impl AsyncService {
///     pub async fn fetch_data(&self, url: String) -> String {
///         format!("Fetched from {}", url)
///     }
/// }
/// ```
#[test]
fn test_async_method_doc() {
    // 这个测试在 doc 中展示
}

/// 复杂返回类型示例
///
/// ```rust,ignore
/// use tokitai::tool;
/// use std::collections::HashMap;
///
/// #[derive(Default)]
/// pub struct DataProcessor;
///
/// #[tool]
/// impl DataProcessor {
///     pub fn get_map(&self) -> HashMap<String, String> {
///         let mut map = HashMap::new();
///         map.insert("key".to_string(), "value".to_string());
///         map
///     }
/// }
/// ```
#[test]
fn test_complex_return_doc() {
    // 这个测试在 doc 中展示
}

/// tool_type 宏示例
///
/// ```rust,ignore
/// use tokitai_macros::tool_type;
///
/// #[tool_type(
///     name = "Location",
///     properties = "latitude: number, longitude: number",
///     required = "latitude, longitude"
/// )]
/// pub struct Location {
///     pub latitude: f64,
///     pub longitude: f64,
/// }
/// ```
#[test]
fn test_tool_type_doc() {
    // 这个测试在 doc 中展示
}
