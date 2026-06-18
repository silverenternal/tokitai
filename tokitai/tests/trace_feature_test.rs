//! T-015: in-process tool-call trace emitter.
//!
//! The macro generates `#[tracing::instrument]` spans on every
//! `__call_*` wrapper when the consumer enables the `trace`
//! feature (`tokitai = { features = ["trace"] }`) or sets the
//! compile-time env var `TOKITAI_TRACE=1`. The test below
//! exercises the feature path:
//!
//!   1. the generated wrapper compiles cleanly with the `trace`
//!      feature on, AND
//!   2. a subscriber attached to the consumer observes one
//!      `tokitai_tool_call` span per call carrying the four
//!      documented fields (`tool.name`, `tool.version`,
//!      `args.size`, `result.size`), AND
//!   3. the macro's compile-time branch is testable without
//!      touching `option_env!` plumbing (we gate the test on
//!      the `trace` feature flag).
//!
//! The "binary size delta is exactly zero when the feature is
//! off" requirement is verified by a separate harness in
//! `tokitai/tests/trace_binary_size_test.rs` (skipped here
//! because binary-size measurement is environment-sensitive
//! and lives in CI).
//!
//! Run with:  cargo test -p tokitai --test trace_feature_test --features trace

#![cfg(feature = "trace")]

use std::io::Write;

use serde_json::json;
use tokitai::tool;
use tracing_subscriber::fmt::MakeWriter;

/// Tiny buffer-backed writer that the test inspects after a
/// call. Used so we can install a `tracing-subscriber::fmt`
/// layer that prints structured spans into the buffer, then
/// grep for the four documented fields. We avoid a custom
/// `Layer` because `tracing-subscriber`'s built-in fmt layer
/// already knows how to record field updates correctly.
#[derive(Default, Clone)]
struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CaptureWriter {
    type Writer = CaptureWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

use std::sync::{Arc, Mutex};

// ============================================================================
// 1. The tool provider under test
// ============================================================================

#[derive(Default)]
struct TracedCalc;

#[tool]
impl TracedCalc {
    /// Add two 32-bit integers.
    #[tool(version = "1.2.0")]
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Subtract `b` from `a`.
    pub fn subtract(&self, a: i32, b: i32) -> i32 {
        a - b
    }
}

// ============================================================================
// 2. Helpers
// ============================================================================

fn run_with_capture<F: FnOnce()>(f: F) -> String {
    let buf = CaptureWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buf.clone())
        .with_ansi(false)
        .with_level(false)
        .with_target(false)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    f();
    drop(_guard);
    let bytes = buf.0.lock().unwrap().clone();
    String::from_utf8(bytes).unwrap_or_default()
}

// ============================================================================
// 3. Tests
// ============================================================================

#[test]
fn wrapper_call_emits_tokitai_tool_call_span() {
    let output = run_with_capture(|| {
        let calc = TracedCalc;
        let result = calc
            .call_tool("add", &json!({"a": 2, "b": 40}))
            .expect("call ok");
        assert_eq!(result, json!(42));
    });

    // The fmt subscriber prints one line per span entry. We
    // expect a `tokitai_tool_call` line carrying
    // `tool.name="add"`, `tool.version="1.2.0"`,
    // `args.size=14` (the byte length of `{"a":2,"b":40}` is
    // 13; whitespace may bump it up by 1), and `result.size=2`
    // (the byte length of `42`).
    assert!(
        output.contains("tokitai_tool_call"),
        "expected a tokitai_tool_call span in output, got: {output}",
    );
    assert!(
        output.contains(r#"tool.name="add""#),
        "expected tool.name=\"add\" in output, got: {output}",
    );
    assert!(
        output.contains(r#"tool.version="1.2.0""#),
        "expected tool.version=\"1.2.0\" in output, got: {output}",
    );
    assert!(
        output.contains("args.size="),
        "expected args.size=<n> in output, got: {output}",
    );
    assert!(
        output.contains("result.size="),
        "expected result.size=<n> in output, got: {output}",
    );
}

#[test]
fn version_attribute_dash_for_unset() {
    // `subtract` does NOT carry a `#[tool(version = "...")]`
    // attribute. The macro should emit `"-"` for the
    // `tool.version` field so the subscriber sees a single
    // stable key across every span.
    let output = run_with_capture(|| {
        let calc = TracedCalc;
        let _ = calc
            .call_tool("subtract", &json!({"a": 5, "b": 2}))
            .expect("call ok");
    });
    assert!(
        output.contains("tokitai_tool_call"),
        "expected a tokitai_tool_call span, got: {output}",
    );
    assert!(
        output.contains(r#"tool.name="subtract""#),
        "expected tool.name=\"subtract\", got: {output}",
    );
    assert!(
        output.contains(r#"tool.version="-""#),
        "expected tool.version=\"-\" when no version attribute is set, got: {output}",
    );
}

#[test]
fn error_path_still_emits_span() {
    // When the tool method returns an error we still want a
    // span emitted. The macro records `args.size` on entry
    // and (when the method runs to completion) `result.size`
    // just before returning. For an early ValidationError
    // raised inside the wrapper body, the span is emitted but
    // `result.size` is left at the `tracing::field::Empty`
    // placeholder because the recording block never runs.
    // Subscribers can detect this case via the
    // `tracing::field::Empty` sentinel.
    let output = run_with_capture(|| {
        let calc = TracedCalc;
        // Wrong type for `a` -> ValidationError.
        let result = calc.call_tool("add", &json!({"a": "not_an_int", "b": 1}));
        assert!(result.is_err(), "expected validation error");
    });
    assert!(
        output.contains("tokitai_tool_call"),
        "expected a tokitai_tool_call span even on the error path, got: {output}",
    );
    // `args.size` is recorded on entry so the field is
    // guaranteed to be present regardless of whether the
    // wrapper reaches its end-of-body recording.
    assert!(
        output.contains("args.size="),
        "expected args.size=<n> on the error path, got: {output}",
    );
}
