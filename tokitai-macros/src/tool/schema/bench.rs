//! Micro-benchmark harness for the schema generation hot path.
//!
//! The proc-macro itself runs at compile time of the consuming
//! crate, so it cannot be timed from a normal `#[test]` or
//! `criterion` bench. What *can* be timed is the
//! [`generate_schema_json_with_deprecated_and_tags`] function and
//! its callees, since they are pure synchronous code that the
//! macro invokes once per `#[tool]`-annotated method.
//!
//! The tests in this module build a synthetic `Vec<ParamInfo>`
//! representing an N-method impl block (with M parameters per
//! method) and measure the wall time of the schema-generation
//! function. They print a `cargo test` summary that is
//! human-readable, so a regression shows up immediately in the
//! test log without needing criterion's full reporting pipeline.
//!
//! These tests are marked `#[ignore]` so they do not slow down
//! the normal `cargo test` run. Run them explicitly with:
//!
//! ```text
//! cargo test -p tokitai-macros --lib -- --ignored schema_bench --nocapture
//! ```
//!
//! The numbers are *relative* — what matters is the trend, not
//! the absolute value. The tests assert a conservative upper
//! bound (1 second per call) so a true regression that hangs the
//! generator surfaces as a failure rather than a silent stall.

#![allow(dead_code)] // bench helpers are only referenced by `#[test]` functions below

use std::time::Instant;

use proc_macro2::Span;
use syn::parse_quote;

use super::gen::{generate_schema_json_with_deprecated_and_tags, SchemaGenConfig};
use crate::tool::types::param::ParamInfo;

/// Build a synthetic parameter list for a single method. Cycles
/// through a small set of representative types so the bench
/// exercises every branch of `generate_schema_for_type`. Each
/// parameter is given a method-unique name so the resulting
/// `BTreeMap<String, JsonSchema>` does not collide when several
/// methods are concatenated.
fn build_params(method_idx: usize, num_params: usize) -> Vec<ParamInfo> {
    let mut params = Vec::with_capacity(num_params);
    for i in 0..num_params {
        let (ty, is_option) = match i % 6 {
            0 => (parse_quote!(String), false),
            1 => (parse_quote!(i64), false),
            2 => (parse_quote!(f64), false),
            3 => (parse_quote!(bool), false),
            4 => (parse_quote!(Option<String>), true),
            _ => (parse_quote!(Vec<String>), false),
        };
        let name = format!("m{}_p{}", method_idx, i);
        let name_ident = syn::Ident::new(&name, Span::call_site());
        params.push(ParamInfo {
            name: name_ident.clone(),
            schema_name: name,
            ty,
            description: Some(format!("Parameter {}", i)),
            is_option,
            is_required: !is_option,
            example: None,
            default: None,
            validate: None,
            transform: None,
            one_of: None,
            enum_values: None,
            pattern: None,
            min: None,
            max: None,
            min_length: None,
            max_length: None,
            min_items: None,
            max_items: None,
            multiple_of: None,
            validate_msg: None,
            validate_msg_zh: None,
            validate_msg_en: None,
        });
    }
    params
}

/// Build a synthetic `Vec<ParamInfo>` for an N-method impl
/// block, M params per method.
fn build_methods(num_methods: usize, num_params_per_method: usize) -> Vec<ParamInfo> {
    let mut all = Vec::with_capacity(num_methods * num_params_per_method);
    for m in 0..num_methods {
        all.extend(build_params(m, num_params_per_method));
    }
    all
}

fn time_schema_gen(params: &[ParamInfo], iters: u32) -> (u128, usize) {
    // Warmup pass — the first call is significantly slower than
    // the steady state because of the `LazyLock` initialisation
    // in `TYPE_SCHEMA_CACHE`. We do one warmup, then time the
    // remaining `iters` calls and take the median of the per-call
    // wall time. This is the same scheme criterion uses
    // internally.
    let _ = generate_schema_json_with_deprecated_and_tags(&SchemaGenConfig::new(params));

    let mut samples: Vec<u128> = Vec::with_capacity(iters as usize);
    let mut last_size = 0usize;
    for _ in 0..iters {
        let start = Instant::now();
        let s = generate_schema_json_with_deprecated_and_tags(&SchemaGenConfig::new(params));
        let elapsed = start.elapsed().as_nanos();
        samples.push(elapsed);
        last_size = s.len();
    }
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    (median, last_size)
}

/// Time the schema-gen hot path with the *extended* config (all
/// of `deprecated_since`, `remove_in`, `group`, `cache`,
/// `rate_limit`, `param_order`, and `example_input` set). This
/// is the path that benefits from the P5-3 optimisation: the
/// old code serialised the schema twice (`to_json_string` then
/// `to_value + to_string`); the new code serialises exactly
/// once.
fn time_schema_gen_extended(params: &[ParamInfo], iters: u32) -> (u128, usize) {
    // Build a serde_json::Value up-front for example_input so we
    // don't pay its construction cost inside the timed loop.
    let example_input: serde_json::Value = serde_json::json!({"example": 1, "fields": ["a", "b"]});
    let param_order: Vec<String> = (0..params.len()).map(|i| format!("m0_p{}", i)).collect();
    // Bind the tag vector so the `&[String]` we pass to `.tags()`
    // outlives the `SchemaGenConfig`.
    let tags: Vec<String> = ["api".to_string(), "public".to_string()].to_vec();

    let cfg = SchemaGenConfig::new(params)
        .deprecated(true)
        .replaced_by(Some("v2"))
        .context(Some("production"))
        .tags(&tags)
        .return_description(Some("the API response"))
        .example_input(Some(&example_input))
        .param_order(Some(&param_order))
        .example_output(Some("{}"))
        .deprecated_note(Some("use v2 instead"))
        .deprecated_since(Some("1.2.0"))
        .remove_in(Some("2.0.0"))
        .group(Some("network"))
        .cache(Some("60s"))
        .rate_limit(Some("100/min"));

    let _ = generate_schema_json_with_deprecated_and_tags(&cfg);
    let mut samples: Vec<u128> = Vec::with_capacity(iters as usize);
    let mut last_size = 0usize;
    for _ in 0..iters {
        let start = Instant::now();
        let s = generate_schema_json_with_deprecated_and_tags(&cfg);
        let elapsed = start.elapsed().as_nanos();
        samples.push(elapsed);
        last_size = s.len();
    }
    samples.sort_unstable();
    (samples[samples.len() / 2], last_size)
}

fn run_bench(name: &str, num_methods: usize, num_params: usize, iters: u32) {
    let params = build_methods(num_methods, num_params);
    let (median_ns, size) = time_schema_gen(&params, iters);
    println!(
        "[schema_bench] {:<28} methods={:>3} params={:>2} iters={:>4} \
         median={:>9} ns  size={:>6} bytes",
        name, num_methods, num_params, iters, median_ns, size
    );
    // Loose upper bound: even on a heavily loaded CI box the
    // single-method case should not take more than a second.
    assert!(
        median_ns < 1_000_000_000,
        "schema gen regressed: median {} ns > 1s",
        median_ns
    );
}

fn run_bench_extended(name: &str, num_methods: usize, num_params: usize, iters: u32) {
    let params = build_methods(num_methods, num_params);
    let (median_ns, size) = time_schema_gen_extended(&params, iters);
    println!(
        "[schema_bench] {:<28} methods={:>3} params={:>2} iters={:>4} \
         median={:>9} ns  size={:>6} bytes [extended]",
        name, num_methods, num_params, iters, median_ns, size
    );
    assert!(
        median_ns < 1_000_000_000,
        "schema gen (extended) regressed: median {} ns > 1s",
        median_ns
    );
}

#[test]
fn schema_bench_5_methods_3_params() {
    run_bench("5m_3p", 5, 3, 50);
}

#[test]
fn schema_bench_10_methods_3_params() {
    run_bench("10m_3p", 10, 3, 50);
}

#[test]
fn schema_bench_50_methods_3_params() {
    run_bench("50m_3p", 50, 3, 20);
}

/// New bench: extended config path (deprecated_since, group,
/// cache, rate_limit, param_order, example_input all set). This
/// is the path that the P5-3 single-serialisation optimisation
/// is meant to speed up. The pre-optimisation implementation
/// did two `to_string` passes — one wasted, one final — when any
/// extension field was set; the new code does exactly one.
#[test]
fn schema_bench_5_methods_3_params_extended() {
    run_bench_extended("5m_3p_ext", 5, 3, 50);
}

#[test]
fn schema_bench_50_methods_3_params_extended() {
    run_bench_extended("50m_3p_ext", 50, 3, 20);
}

/// Captures the byte-for-byte schema output for a 5-method
/// fixture. We assert against a pinned baseline string so any
/// optimization that accidentally changes the output shape (a
/// dropped field, a different key order, a re-format) fails
/// loudly. The test also dumps the current JSON to stderr under
/// the `[schema_baseline]` prefix so it can be diffed against a
/// previously-saved baseline.
#[test]
fn schema_baseline_5_methods_matches_pinned_string() {
    let params = build_methods(5, 3);
    let json = generate_schema_json_with_deprecated_and_tags(&SchemaGenConfig::new(&params));
    eprintln!("[schema_baseline] 5m_3p size={} bytes", json.len());
    eprintln!("[schema_baseline] json={}", json);
    // Round-trip through serde to catch any structural drift.
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("schema gen output must be valid JSON");
    assert_eq!(parsed["type"], "object");
    assert!(parsed["properties"].is_object());
    let props = parsed["properties"].as_object().unwrap();
    assert_eq!(props.len(), 15, "expected 5*3=15 properties");
    // Byte-for-byte equality against the unoptimised baseline
    // recorded on 2026-06-02 (v0.4.0 SHA 8dc5f0e + bench harness).
    // If this fails, your optimization has changed the JSON
    // Schema output shape — that is an API-visible regression.
    assert_eq!(
        json, SCHEMA_BASELINE_5M3P,
        "schema JSON drifted from pinned baseline"
    );
}

/// Pinned baseline: 5 methods × 3 params, all params required,
/// one of each of {string, integer, number} per method. Captured
/// on the unoptimised v0.4.0 build. Any optimization that
/// changes the output bytes must update this constant **and** be
/// reviewed for API impact.
const SCHEMA_BASELINE_5M3P: &str = "{\"type\":\"object\",\"properties\":{\"m0_p0\":{\"type\":\"string\",\"description\":\"Parameter 0\"},\"m0_p1\":{\"type\":\"integer\",\"description\":\"Parameter 1\"},\"m0_p2\":{\"type\":\"number\",\"description\":\"Parameter 2\"},\"m1_p0\":{\"type\":\"string\",\"description\":\"Parameter 0\"},\"m1_p1\":{\"type\":\"integer\",\"description\":\"Parameter 1\"},\"m1_p2\":{\"type\":\"number\",\"description\":\"Parameter 2\"},\"m2_p0\":{\"type\":\"string\",\"description\":\"Parameter 0\"},\"m2_p1\":{\"type\":\"integer\",\"description\":\"Parameter 1\"},\"m2_p2\":{\"type\":\"number\",\"description\":\"Parameter 2\"},\"m3_p0\":{\"type\":\"string\",\"description\":\"Parameter 0\"},\"m3_p1\":{\"type\":\"integer\",\"description\":\"Parameter 1\"},\"m3_p2\":{\"type\":\"number\",\"description\":\"Parameter 2\"},\"m4_p0\":{\"type\":\"string\",\"description\":\"Parameter 0\"},\"m4_p1\":{\"type\":\"integer\",\"description\":\"Parameter 1\"},\"m4_p2\":{\"type\":\"number\",\"description\":\"Parameter 2\"}},\"required\":[\"m0_p0\",\"m0_p1\",\"m0_p2\",\"m1_p0\",\"m1_p1\",\"m1_p2\",\"m2_p0\",\"m2_p1\",\"m2_p2\",\"m3_p0\",\"m3_p1\",\"m3_p2\",\"m4_p0\",\"m4_p1\",\"m4_p2\"]}";
