//! 工具方法信息数据结构

use proc_macro2::Span;
use serde_json::Value;
use syn::ReturnType;

use super::param::{ParamInfo, ParamToolAttrs};

/// 工具方法信息
#[allow(dead_code)]
pub struct ToolMethodInfo {
    /// Span of the method's identifier in the user's source.
    /// T-001: every `compile_error!` emitted by `#[tool]` is
    /// anchored at this span (or a sub-span of it) so editors
    /// jump to the user's offending token rather than the
    /// generated code inside the macro expansion.
    pub ident_span: Span,
    pub name: String,
    pub tool_name: String,
    pub description: String,
    pub params: Vec<ParamInfo>,
    pub is_async: bool,
    pub is_result: bool,
    pub is_generic: bool,
    pub deprecated: bool,
    pub replaced_by: Option<String>,
    pub deprecated_note: Option<String>,
    pub deprecated_since: Option<String>,
    pub remove_in: Option<String>,
    pub version: Option<String>,
    pub visible: bool,
    pub tags: Vec<String>,
    pub group: Option<String>,
    pub return_description: Option<String>,
    pub context: Option<String>,
    pub example_input: Option<Value>,
    pub param_order: Option<Vec<String>>,
    pub hidden_params: Vec<String>,
    pub example_output: Option<String>,
    pub return_type: ReturnType,
    pub doc: Option<String>,
    pub alias: Vec<String>,
    pub allow: Vec<String>,
    pub cache: Option<String>,
    pub rate_limit: Option<String>,
    pub param_validations: Vec<(String, ParamToolAttrs)>,
    /// T-002: `true` when the description was supplied explicitly via
    /// `#[tool(desc = "...")]`. When `true` the runtime `tokitai!`
    /// config block must NOT override the description — see
    /// `tokitai_core::config::CONFIG_PRIORITY_ORDER`.
    pub description_explicit: bool,
}
