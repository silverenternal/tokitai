//! 运行时错误类型

use thiserror::Error;

/// AI 工具调用错误
#[derive(Error, Debug)]
pub enum AiToolError {
    /// 参数验证错误
    #[error("验证错误：{message}")]
    ValidationError { message: String },

    /// 工具未找到
    #[error("工具未找到：{name}")]
    NotFound { name: String },

    /// 序列化错误
    #[error("序列化错误：{0}")]
    SerializationError(#[from] serde_json::Error),

    /// 内部错误
    #[error("内部错误：{message}")]
    InternalError { message: String },
}

impl From<AiToolError> for tokitai_core::ToolError {
    fn from(err: AiToolError) -> Self {
        match err {
            AiToolError::ValidationError { message } => {
                tokitai_core::ToolError::validation_error(message)
            }
            AiToolError::NotFound { name } => {
                tokitai_core::ToolError::not_found(name)
            }
            AiToolError::SerializationError(e) => {
                tokitai_core::ToolError::internal_error(e.to_string())
            }
            AiToolError::InternalError { message } => {
                tokitai_core::ToolError::internal_error(message)
            }
        }
    }
}
