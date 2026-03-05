//! MCP (Model Context Protocol) 支持
//!
//! 提供与 MCP 兼容的工具定义格式。
//!
//! # 使用示例
//!
//! ```rust,ignore
//! use tokitai::{tool, mcp};
//!
//! #[tool]
//! impl Calculator {
//!     pub fn add(&self, a: i32, b: i32) -> i32 {
//!         a + b
//!     }
//! }
//!
//! // 转换为 MCP 格式
//! let mcp_tools = mcp::to_mcp_tools(Calculator::TOOL_DEFINITIONS);
//! ```

use serde::{Deserialize, Serialize};
use tokitai_core::ToolDefinition;

/// MCP 工具定义格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// 将 tokitai 工具定义转换为 MCP 格式
pub fn to_mcp_tools(tools: &[ToolDefinition]) -> Vec<McpTool> {
    tools
        .iter()
        .map(|t| McpTool {
            name: t.name.to_string(),
            description: t.description.to_string(),
            input_schema: serde_json::from_str(t.input_schema).unwrap_or_default(),
        })
        .collect()
}

/// MCP 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// MCP 工具调用响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl McpToolResponse {
    pub fn success(result: serde_json::Value) -> Self {
        Self {
            success: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(message.into()),
        }
    }
}
