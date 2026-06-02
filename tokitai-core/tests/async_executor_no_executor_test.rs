//! Integration test for the `AsyncExecutor` "no executor registered" error
//! path. This test is intentionally in its own binary so the process-wide
//! `ASYNC_EXECUTOR` `OnceLock` is guaranteed to be empty when the test
//! runs. The other executor tests live in `src/lib.rs` and share the same
//! binary.
//!
//! Run with: `cargo test -p tokitai-core --test async_executor_no_executor_test --features serde`

#![cfg(feature = "serde")]

use tokitai_core::{block_on_async, block_on_async_error_message, ToolError, ToolErrorKind};

#[test]
fn test_block_on_async_no_executor() {
    let result: Result<(), ToolError> = block_on_async(async {});
    let err = result.expect_err("block_on_async should error when no AsyncExecutor is registered");
    assert_eq!(err.kind, ToolErrorKind::InternalError);
    let expected = block_on_async_error_message();
    assert!(
        err.message.contains(expected) || err.message.contains("no async runtime"),
        "expected English error message, got: {:?}",
        err.message
    );
}
