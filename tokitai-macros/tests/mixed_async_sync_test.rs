//! Mixed async + sync methods in the same impl block.
//!
//! Regression target for Bug 1: a previous version of the macro failed
//! to compile when an `impl` block contained both `async fn` and
//! `fn` methods, because the dispatcher shared a single `has_async`
//! flag across all methods and emitted only one of the two wrapper
//! flavours.
//!
//! With the fix, a mixed impl generates:
//! - `pub async fn call_tool(...)` (because at least one method is async)
//! - `pub fn call_tool_sync(...)` (so sync callers can drive async tools)
//! - For each async method: both `__call_<name>` (async) and `__call_<name>_sync` (sync)
//! - For each sync method: only `__call_<name>_sync` (sync)
//!
//! The test runs inside a `#[tokio::main]` runtime so async methods
//! have a current `Handle`. We then drive the `call_tool_sync`
//! dispatcher (which is the path that handles a mixed impl) and the
//! direct sync wrapper for an async method.

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct MixedTools;

#[tool]
impl MixedTools {
    /// Async method: returns a string after a brief await.
    pub async fn async_echo(&self, text: String) -> String {
        format!("async:{}", text)
    }

    /// Sync method: pure compute, no I/O.
    pub fn sync_add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Async method that uses the default runtime.
    pub async fn async_concat(&self, prefix: String, suffix: String) -> String {
        format!("{}{}", prefix, suffix)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Sanity: the impl block compiles and registers 3 tools.
    assert_eq!(MixedTools::tool_definitions().len(), 3);

    let tools = MixedTools;

    // 1) call_tool_sync: drives the async methods through tokio
    //    and calls the sync method directly. This is the dispatcher
    //    path that must work for a mixed impl.
    let r = tools
        .call_tool_sync("async_echo", &serde_json::json!({"text": "world"}))
        .expect("async_echo via call_tool_sync");
    assert_eq!(r, serde_json::json!("async:world"));

    let r = tools
        .call_tool_sync("sync_add", &serde_json::json!({"a": 10, "b": 20}))
        .expect("sync_add via call_tool_sync");
    assert_eq!(r, serde_json::json!(30));

    let r = tools
        .call_tool_sync(
            "async_concat",
            &serde_json::json!({"prefix": "x", "suffix": "y"}),
        )
        .expect("async_concat via call_tool_sync");
    assert_eq!(r, serde_json::json!("xy"));

    // 2) Direct sync wrapper on the async method: should succeed in
    //    a tokio runtime (Handle::try_current() returns Ok).
    let r = tools
        .__call_async_echo_sync(&serde_json::json!({"text": "direct"}))
        .expect("direct __call_async_echo_sync");
    assert_eq!(r, serde_json::json!("async:direct"));

    // 3) Inherent async `call_tool` on a mixed impl must work: it
    //    dispatches the async method via its async wrapper and
    //    drives the sync method through `async { ... }.await`.
    //    We are inside `#[tokio::main]` so `.await` is fine.
    let r = tools
        .call_tool(
            "async_concat",
            &serde_json::json!({"prefix": "p", "suffix": "q"}),
        )
        .await
        .expect("async_concat via inherent async call_tool");
    assert_eq!(r, serde_json::json!("pq"));
}
