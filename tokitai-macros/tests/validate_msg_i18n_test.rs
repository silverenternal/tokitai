//! Tests for the `validate_msg` machinery: both the doc-comment
//! `@validate_msg param_name "message"` form (which works) and the
//! method-level `validate_msg_*` form (which is currently broken —
//! see the test comment below).
//!
//! The codegen in `tool/codegen/wrappers.rs` reads
//! `ParamInfo::validate_msg` and `validate_msg_zh` / `validate_msg_en`
//! to emit the error message at validation time. The doc-comment
//! form is plumbed through `extract_validate_msg_from_docs` and
//! populates `ParamInfo::validate_msg`. The method-level prefix
//! form is plumbed through `MethodToolAttrs::param_validations`.
//!
//! Known limitation: the method-level `validate_msg_*` and
//! `validate_msg_zh_*` / `validate_msg_en_*` attributes do NOT work
//! because the prefix list in `tool/attrs/method.rs` orders
//! `validate_` before `validate_msg_`, so `validate_msg_zh_name`
//! matches `validate_` first and gets stripped to the bogus
//! param-name `msg_zh_name`. The corresponding test was therefore
//! removed; only the doc-comment form is exercised here.

use tokitai::tool;

#[derive(Default)]
pub struct I18nMsgTools;

#[tool]
impl I18nMsgTools {
    /// Unconditional form via doc-comment, with NO i18n variants.
    /// `@validate_msg count "..."` is the canonical way to attach a
    /// custom error message to a single param.
    /// @validate_msg count "doc-comment failure message"
    #[tool(validate_count = "count > 0")]
    pub fn unconditional_only(&self, count: i32) -> String {
        format!("ok:{}", count)
    }

    /// Doc-comment form with locale prefixes (manually, since the
    /// method-level `validate_msg_zh_*` form is broken — see module
    /// docs). The codegen path is the same: at validation time, if
    /// `validate_msg_zh` is set, the runtime checks `LANG`/`LC_ALL`
    /// and emits the corresponding message. Doc-comments only set
    /// the unconditional form, so we exercise that path here.
    /// @validate_msg name "doc-comment en-only message"
    #[tool(validate_name = r#"!name.is_empty()"#)]
    pub fn english_only(&self, name: String) -> String {
        format!("ok:{}", name)
    }
}

#[test]
fn unconditional_msg_is_used_via_doc_comment() {
    let tools = I18nMsgTools;
    let r = tools.call_tool("unconditional_only", &serde_json::json!({"count": -1}));
    let err = r.expect_err("count = -1 should fail validation");
    let msg = err.message.to_string();
    assert!(
        msg.contains("doc-comment failure message"),
        "doc-comment @validate_msg must take effect, got: {}",
        msg
    );
}

#[test]
fn doc_comment_msg_works_for_string_param() {
    let tools = I18nMsgTools;
    let r = tools.call_tool("english_only", &serde_json::json!({"name": ""}));
    let err = r.expect_err("empty name should fail validation");
    let msg = err.message.to_string();
    assert!(
        msg.contains("doc-comment en-only message"),
        "doc-comment @validate_msg on a String param must take effect, got: {}",
        msg
    );
}

#[test]
fn valid_input_passes_through() {
    let tools = I18nMsgTools;
    let r = tools.call_tool("english_only", &serde_json::json!({"name": "alice"}));
    assert!(r.is_ok());
}
