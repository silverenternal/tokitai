//! T-001: Macro error spans must point at user code, not generated code.
//!
//! These tests are the structural counterpart of the trybuild
//! `compile_fail` snapshots in `tests/ui/`. The trybuild fixtures
//! verify that rustc reports a diagnostic at *some* line/column;
//! this test verifies the more load-bearing contract: the
//! `compile_error!` is anchored at the user-written token (the
//! method name, the parameter, the attribute) rather than at the
//! `#[tool]` attribute or at a generated `__call_*` symbol.
//!
//! How the test works:
//!
//! 1. Each negative case feeds a source string literal to a hidden
//!    `__error_spans!` proc-macro that runs the same validation
//!    pipeline as the real `#[tool]` macro and returns the
//!    `&'static str` rendering of each emitted `compile_error!`.
//! 2. The rendering contains a `quote_spanned!`-injected span
//!    alongside the offending token's source, so the test can
//!    assert that the user-written token (`method_without_self`,
//!    `not_a_real_attr`, `fn_ptr_return`, ...) appears in the
//!    output — proving the macro picked up the user's token as
//!    the error's anchor.
//! 3. A *negative* assertion confirms the diagnostic does NOT
//!    point at `__call_*` (a generated-only symbol), so a future
//!    refactor that regresses the span to the macro call site
//!    trips this test loudly.
//!
//! See `tests/ui/07_missing_self_param.stderr` and
//! `tests/ui/08_generic_method.stderr` for the matching trybuild
//! snapshots that show the same span stabilised at the
//! user-written token.

use tokitai_macros::__error_spans;

#[test]
fn missing_self_span_anchors_on_method_name() {
    // The user wrote `pub fn method_without_self(...)` — the
    // diagnostic must mention the method *name* and must NOT
    // mention a generated symbol.
    let report: &'static str = __error_spans!(
        r#"
        struct __SpanProbe;
        impl __SpanProbe {
            pub fn method_without_self(a: i32, b: i32) -> i32 { a + b }
        }
        "#
    );
    assert!(
        report.contains("E0012"),
        "expected E0012 (missing self) in report, got:\n{}",
        report
    );
    assert!(
        report.contains("method_without_self"),
        "diagnostic must reference the user-written method name, got:\n{}",
        report
    );
    // Regression guard: must not point at a generated symbol.
    assert!(
        !report.contains("__call_method_without_self"),
        "diagnostic must not point at a generated __call_* symbol, got:\n{}",
        report
    );
}

#[test]
fn invalid_return_type_span_anchors_on_type() {
    // E0021: return type `fn(i32) -> i32` is not schemable.
    // The diagnostic must reference the method *name*; the
    // span itself sits on the return type.
    let report: &'static str = __error_spans!(
        r#"
        struct __SpanProbe;
        impl __SpanProbe {
            pub fn fn_ptr_return(&self) -> fn(i32) -> i32 { |x| x }
        }
        "#
    );
    assert!(
        report.contains("E0021"),
        "expected E0021 (unsupported return type) in report, got:\n{}",
        report
    );
    assert!(
        report.contains("fn_ptr_return"),
        "diagnostic must reference the user-written method name, got:\n{}",
        report
    );
    assert!(
        report.contains("bare function pointer") || report.contains("fn("),
        "diagnostic must mention the offending return-type shape, got:\n{}",
        report
    );
}

#[test]
fn generic_method_span_anchors_on_method_name() {
    // E0004: generic methods are not supported. The diagnostic
    // must reference the user-written method *name* — the same
    // span that the trybuild snapshot in
    // `tests/ui/08_generic_method.stderr` anchors on line 11.
    let report: &'static str = __error_spans!(
        r#"
        struct __SpanProbe;
        impl __SpanProbe {
            pub fn generic_method<T: ToString>(&self, value: T) -> String { value.to_string() }
        }
        "#
    );
    // Generic methods are caught by `extract_tool_info`
    // (returning `is_generic: true`) and surfaced by
    // `definitions::generate_tool_def_consts` — the same path
    // the trybuild 08_generic_method fixture exercises.
    // Either the validation pipeline (E0004) or the
    // codegen-time compile_error! path may produce the
    // diagnostic; we accept either, but the user-written
    // method name must appear in the rendered output.
    assert!(
        report.contains("generic_method"),
        "diagnostic must reference the user-written method name, got:\n{}",
        report
    );
    // The rendered output must not point at a generated symbol.
    assert!(
        !report.contains("__call_generic_method") && !report.contains("__TOOL_DEF_GENERIC_METHOD"),
        "diagnostic must not point at a generated symbol, got:\n{}",
        report
    );
}

#[test]
fn valid_impl_produces_no_errors() {
    // Sanity check: a well-formed impl produces an empty
    // report (the proc-macro returns an empty string literal
    // when `validate_impl` reports no problems).
    let report: &'static str = __error_spans!(
        r#"
        struct __SpanProbe;
        impl __SpanProbe {
            pub fn ok(&self, a: i32) -> i32 { a }
        }
        "#
    );
    assert!(
        report.trim().is_empty(),
        "expected no diagnostics for a well-formed impl, got:\n{}",
        report
    );
}
