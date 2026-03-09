//! Runtime error types

use thiserror::Error;

/// AI tool invocation error
///
/// This error type wraps various failures that can occur during tool execution.
///
/// # Examples
///
/// ```rust,ignore
/// use tokitai::AiToolError;
///
/// // Validation error
/// let err = AiToolError::ValidationError {
///     message: "Missing required parameter 'city'".to_string(),
/// };
///
/// // Tool not found
/// let err = AiToolError::NotFound {
///     name: "unknown_tool".to_string(),
/// };
/// ```
#[derive(Error, Debug)]
pub enum AiToolError {
    /// Parameter validation error
    #[error("Validation error: {message}")]
    ValidationError { message: String },

    /// Tool not found
    #[error("Tool not found: {name}")]
    NotFound { name: String },

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Internal error
    #[error("Internal error: {message}")]
    InternalError { message: String },
}

impl From<AiToolError> for tokitai_core::ToolError {
    fn from(err: AiToolError) -> Self {
        match err {
            AiToolError::ValidationError { message } => {
                tokitai_core::ToolError::validation_error(message)
            }
            AiToolError::NotFound { name } => tokitai_core::ToolError::not_found(name),
            AiToolError::SerializationError(e) => {
                tokitai_core::ToolError::internal_error(e.to_string())
            }
            AiToolError::InternalError { message } => {
                tokitai_core::ToolError::internal_error(message)
            }
        }
    }
}
