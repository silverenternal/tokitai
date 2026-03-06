//! AI 客户端模块
//!
//! 简化的 AI 客户端示例，实际使用时可替换为真实的 AI API

use serde_json::Value;

/// AI 客户端 trait
///
/// 实现此 trait 以集成不同的 AI 服务
pub trait AIClient {
    /// 发送消息并获取响应
    fn chat(&self, message: &str) -> Result<String, String>;

    /// 处理工具调用请求
    fn handle_tool_call(&self, tool_name: &str, args: &Value) -> Result<Value, String>;
}

/// 简单的模拟 AI 客户端（用于演示）
pub struct MockAIClient;

impl AIClient for MockAIClient {
    fn chat(&self, message: &str) -> Result<String, String> {
        // 模拟 AI 响应
        Ok(format!("[AI 模拟响应] 我收到了你的消息：{}", message))
    }

    fn handle_tool_call(&self, tool_name: &str, _args: &Value) -> Result<Value, String> {
        // 这个方法在实际使用时会被替换为真实的工具调用逻辑
        Err(format!("工具调用需要在主程序中实现：{}", tool_name))
    }
}

/// 创建模拟 AI 客户端
pub fn create_mock_client() -> MockAIClient {
    MockAIClient
}
