//! 可选参数测试

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct Greeter;

#[tool]
impl Greeter {
    /// 打招呼，可选的语言参数
    pub fn greet(&self, name: String, language: Option<String>) -> String {
        match language.as_deref() {
            Some("zh") => format!("你好，{}！", name),
            Some("es") => format!("¡Hola, {}!", name),
            _ => format!("Hello, {}!", name),
        }
    }
}

fn main() {
    let greeter = Greeter;

    // 验证 TOOL_DEFINITIONS 生成
    let tools = Greeter::tool_definitions();
    assert_eq!(tools.len(), 1);

    // 不带可选参数
    let result = greeter.call_tool("greet", &serde_json::json!({"name": "Alice"})).unwrap();
    assert_eq!(result, "Hello, Alice!");

    // 带可选参数
    let result = greeter.call_tool("greet", &serde_json::json!({"name": "Bob", "language": "zh"})).unwrap();
    assert_eq!(result, "你好，Bob！");
}
