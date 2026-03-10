//! 测试 09: async 方法支持测试

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct AsyncTools;

#[tool]
impl AsyncTools {
    /// 异步方法应该被支持
    pub async fn async_fetch(&self, url: String) -> String {
        format!("Fetched from {}", url)
    }
}

#[tokio::main]
async fn main() {
    let tools = AsyncTools;
    
    // 验证 TOOL_DEFINITIONS 生成
    let defs = AsyncTools::tool_definitions();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "async_fetch");
    
    // 验证异步调用
    let result = tools.call_tool("async_fetch", &serde_json::json!({"url": "https://example.com"})).await;
    assert!(result.is_ok());
}
