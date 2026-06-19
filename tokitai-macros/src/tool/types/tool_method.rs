//! 工具方法信息数据结构

use proc_macro2::Span;
use serde_json::Value;
use syn::ReturnType;

use super::param::{ParamInfo, ParamToolAttrs};
use crate::tool::example::BakedExample;

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
    /// T-016: baked few-shot examples collected from
    /// `#[tool(example = call!(...))]` or
    /// `#[tool(examples = [call!(...), ...])]`. The codegen layer
    /// inlines them at two sites: a compile-time type-check in
    /// each generated wrapper, and the schema's `examples` field.
    pub baked_examples: Vec<BakedExample>,
    /// T-018: span of the `desc = "..."` literal, if the user
    /// supplied one. The lint uses this span to anchor the
    /// `compile_error!` so the diagnostic points at the literal
    /// in the user's source.
    pub desc_span: Option<Span>,
    /// T-018: per-method minimum description score override.
    /// When `None`, the impl-level threshold (or default 60)
    /// applies.
    pub min_desc_score: Option<u8>,
    /// T-018: per-method opt-out flag. When `true`, the lint is
    /// bypassed for this method regardless of score.
    pub allow_short_desc: bool,
    /// T-022: per-method opt-out flag from the adversarial-
    /// description safety lint. When `true`, this method's
    /// `desc = "..."` literal bypasses the bad-pattern matcher.
    pub allow_insecure_desc: bool,
    /// T-022: per-method extension of the bad-pattern set.
    /// Each entry is a case-insensitive substring; a hit raises
    /// the safety lint for this method only. The macro folds the
    /// per-build env-var entries into the same matcher at lint
    /// time.
    pub desc_blocklist: Vec<String>,
    /// T-019: per-method byte budget for the serialized result.
    /// When `Some(n)`, the macro compiles a runtime guard into
    /// the `__call_*` wrapper. The guard is a no-op when
    /// `None` — every existing tool that does not opt in
    /// continues to behave exactly as it did before T-019.
    pub result_truncate_bytes: Option<usize>,
    /// T-020: lower bound (inclusive) of the schema-evolution
    /// interval. Mirrors `#[tool(since = "...")]`.
    pub since: Option<String>,
    /// T-020: upper bound (exclusive) of the schema-evolution
    /// interval. Mirrors `#[tool(until = "...")]`.
    pub until: Option<String>,
}
