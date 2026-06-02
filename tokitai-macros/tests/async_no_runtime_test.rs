//! Async method called from a sync context without a tokio runtime
//! should fail with the "no tokio runtime" error.
//!
//! This is in its own binary so the test thread is guaranteed not to be
//! inside a tokio runtime (other tests in the workspace that use
//! `#[tokio::test]` or `tokio::runtime::Runtime` would otherwise leave
//! a runtime current on this thread).
//!
//! Regression target: the sync wrapper for an `async fn` tool uses
//! `tokio::runtime::Handle::try_current()` and returns an
//! `InternalError` with a descriptive message if no runtime is in
//! scope. The message must contain the literal "tokio runtime" so
//! downstream observability tools can pattern-match on it.

use tokitai::tool;
use tokitai_core::ToolErrorKind;

#[derive(Default)]
pub struct AsyncNoRuntimeTools;

#[tool]
impl AsyncNoRuntimeTools {
    /// An async tool. The macro must generate both an async
    /// `__call_*` wrapper and a sync `__call_*_sync` wrapper that
    /// runs the future through the current tokio runtime.
    pub async fn fetch(&self, url: String) -> String {
        format!("fetched {}", url)
    }
}

#[test]
fn sync_call_to_async_tool_without_runtime_errors() {
    // We are intentionally not inside a `tokio::runtime::Runtime`
    // here. Calling `call_tool_sync` (the dispatcher generated for
    // any impl that has at least one `async fn` tool) should return
    // an `InternalError` with a clear "no tokio runtime" message.
    let tools = AsyncNoRuntimeTools;
    let result = tools.call_tool_sync("fetch", &serde_json::json!({"url": "https://example.com"}));

    let err = result.expect_err("expected an error when no tokio runtime is available");
    assert_eq!(err.kind, ToolErrorKind::InternalError);
    let msg = err.message.to_string();
    assert!(
        msg.contains("tokio runtime"),
        "error message should mention the missing tokio runtime, got: {:?}",
        msg
    );
}

#[test]
fn sync_wrapper_directly_without_runtime_errors() {
    // Calling the per-method sync wrapper directly should also fail
    // cleanly. This is the path the dispatcher takes.
    let tools = AsyncNoRuntimeTools;
    let result = tools.__call_fetch_sync(&serde_json::json!({"url": "https://example.com"}));

    let err = result.expect_err("expected an error from __call_fetch_sync without a runtime");
    assert_eq!(err.kind, ToolErrorKind::InternalError);
    let msg = err.message.to_string();
    assert!(
        msg.contains("tokio runtime"),
        "error message should mention the missing tokio runtime, got: {:?}",
        msg
    );
}
