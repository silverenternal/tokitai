//! T-019 (companion): tracing::warn fires when the
//! `result_truncate_bytes` budget is exceeded AND the `trace`
//! feature is on.
//!
//! Mirrors `trace_feature_test.rs` (T-015) for the T-019
//! truncation event. The test installs a buffer-backed
//! `tracing_subscriber::fmt` subscriber, calls a tool whose
//! return exceeds the budget, and greps the captured output
//! for the T-019 fields.
//!
//! Run:
//!   cargo test -p tokitai --test result_truncate_trace_test --features trace
//!
//! Note: the test is gated on the `trace` feature so the
//! `tracing_subscriber` / `tracing` dependencies are pulled in
//! only when needed. The default build (no `trace` feature)
//! sees zero `tracing` references in the generated code, so
//! the binary-size delta from T-019 is also zero on the
//! default build.

#![cfg(feature = "trace")]

use std::io::Write;
use std::sync::{Arc, Mutex};

use serde_json::json;
use tokitai::tool;
use tracing_subscriber::fmt::MakeWriter;

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

#[derive(Default)]
struct TraceTruncateProvider;

#[tool]
impl TraceTruncateProvider {
    /// String over the budget. The guard's
    /// `tracing::warn!(...)` event must appear in the
    /// subscriber output.
    #[tool(result_truncate_bytes = 256)]
    pub fn big_string(&self) -> String {
        "x".repeat(1024)
    }
}

fn run_with_capture<F: FnOnce()>(f: F) -> String {
    let buf = CaptureWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buf.clone())
        .with_ansi(false)
        .with_level(true)
        .with_target(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    f();
    drop(_guard);
    let bytes = buf.0.lock().unwrap().clone();
    String::from_utf8(bytes).unwrap_or_default()
}

#[test]
fn warn_event_fires_when_truncation_guard_triggers() {
    let output = run_with_capture(|| {
        let provider = TraceTruncateProvider;
        let _ = provider
            .call_tool("big_string", &json!({}))
            .expect("string truncation should return a value");
    });

    // The T-019 warn event identifies itself by the message
    // string and the two byte-count fields. We grep for both
    // because the exact formatting depends on the subscriber
    // (the test uses the default fmt layer).
    assert!(
        output.contains("tokitai T-019"),
        "expected the T-019 warn event message, got: {output}"
    );
    assert!(
        output.contains("original_bytes"),
        "expected `original_bytes` field in the T-019 warn event, got: {output}"
    );
    assert!(
        output.contains("kept_bytes"),
        "expected `kept_bytes` field in the T-019 warn event, got: {output}"
    );
}
