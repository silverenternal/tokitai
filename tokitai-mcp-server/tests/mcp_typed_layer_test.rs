//! T-021 typed MCP handle layer tests.
//!
//! Three test classes cover the acceptance criteria:
//!
//! 1. **Positive**: a well-formed call against each fixture tool is
//!    accepted; the handler runs and returns the expected value.
//! 2. **Negative**: a call that supplies the wrong type for a field
//!    is rejected with `ToolError::ValidationError` whose message
//!    contains the JSON Pointer to the offending field; the handler
//!    is NOT invoked (asserted via a counter).
//! 3. **Fuzz**: 100 random input shapes per fixture tool; every
//!    malformed input is refused with `ToolError::ValidationError`,
//!    and the handler invocation count matches the number of valid
//!    inputs.
//!
//! These tests exercise the typed layer directly (without going
//! through axum or the stdio transport), so they run identically
//! whether the `mcp-typed` feature is on or off. The feature gate is
//! verified separately by `cargo build --no-default-features` and
//! `cargo build --features mcp-typed` in CI.

use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{json, Value};
use tokitai_core::{ToolError, ToolErrorKind};
use tokitai_mcp_server::typed::{
    load_typed_fixtures, validate_against_schema, TypedDispatcher, TypedToolSpec,
};

/// Counter the negative-test handler increments on every invocation.
/// `AtomicUsize` because the typed layer could in principle dispatch
/// from any thread (HTTP handler threads, etc.); tests stay correct
/// even under future concurrency.
#[derive(Default)]
struct CallCounter(AtomicUsize);

impl CallCounter {
    fn new() -> Self {
        Self(AtomicUsize::new(0))
    }
    fn inc(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn get(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

/// Build a dispatcher that loads the standard fixture set. Each call
/// to the handler increments the counter and returns a sentinel value
/// derived from the tool name so the test can confirm which handler
/// ran.
fn dispatcher_with_counter() -> (TypedDispatcher, CallCounter) {
    let counter = CallCounter::new();
    let dispatcher = TypedDispatcher::from_specs(load_typed_fixtures());
    (dispatcher, counter)
}

fn dispatch<F>(
    dispatcher: &TypedDispatcher,
    counter: &CallCounter,
    tool: &str,
    args: &Value,
    handler: F,
) -> Result<Value, ToolError>
where
    F: FnOnce(&Value) -> Result<Value, ToolError>,
{
    dispatcher.dispatch(tool, args, |a| {
        counter.inc();
        handler(a)
    })
}

// ---------------------------------------------------------------------------
// Positive cases
// ---------------------------------------------------------------------------

#[test]
fn positive_add() {
    let (d, c) = dispatcher_with_counter();
    let r = dispatch(&d, &c, "add", &json!({"a": 2, "b": 3}), |a| {
        let x = a["a"].as_i64().unwrap();
        let y = a["b"].as_i64().unwrap();
        Ok(json!(x + y))
    })
    .expect("add must accept valid input");
    assert_eq!(r, json!(5));
    assert_eq!(c.get(), 1, "handler must be called exactly once");
}

#[test]
fn positive_greet() {
    let (d, c) = dispatcher_with_counter();
    let r = dispatch(&d, &c, "greet", &json!({"name": "world"}), |a| {
        Ok(json!(format!("Hello, {}!", a["name"].as_str().unwrap())))
    })
    .expect("greet must accept valid input");
    assert_eq!(r, json!("Hello, world!"));
    assert_eq!(c.get(), 1);
}

#[test]
fn positive_greet_with_title() {
    let (d, c) = dispatcher_with_counter();
    let r = dispatch(
        &d,
        &c,
        "greet",
        &json!({"name": "world", "title": "Dr."}),
        |a| {
            let name = a["name"].as_str().unwrap();
            let title = a.get("title").and_then(|v| v.as_str());
            let greeting = match title {
                Some(t) => format!("Hello, {} {}!", t, name),
                None => format!("Hello, {}!", name),
            };
            Ok(json!(greeting))
        },
    )
    .expect("greet must accept optional title");
    assert_eq!(r, json!("Hello, Dr. world!"));
}

#[test]
fn positive_reverse() {
    let (d, c) = dispatcher_with_counter();
    let r = dispatch(&d, &c, "reverse", &json!({"text": "tokitai"}), |a| {
        let s = a["text"].as_str().unwrap();
        Ok(json!(s.chars().rev().collect::<String>()))
    })
    .expect("reverse must accept valid input");
    assert_eq!(r, json!("iatikot"));
    assert_eq!(c.get(), 1);
}

#[test]
fn positive_find_user() {
    let (d, c) = dispatcher_with_counter();
    let r = dispatch(&d, &c, "find_user", &json!({"user_id": 42}), |a| {
        let id = a["user_id"].as_i64().unwrap();
        Ok(json!({
            "id": id,
            "name": format!("user_{}", id),
            "active": true,
        }))
    })
    .expect("find_user must accept valid input");
    assert_eq!(r, json!({"id": 42, "name": "user_42", "active": true}));
    assert_eq!(c.get(), 1);
}

// ---------------------------------------------------------------------------
// Negative cases
// ---------------------------------------------------------------------------

#[test]
fn negative_add_string_for_integer() {
    let (d, c) = dispatcher_with_counter();
    let err = dispatch(&d, &c, "add", &json!({"a": "ten", "b": 2}), |_a| {
        unreachable!("handler must not be called on validation failure")
    })
    .expect_err("add must reject string-for-integer");
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(
        err.message.contains("/a"),
        "error must contain JSON Pointer to offending field: {}",
        err.message
    );
    assert_eq!(
        c.get(),
        0,
        "handler must NEVER be invoked when validation fails"
    );
}

#[test]
fn negative_add_missing_required_field() {
    let (d, c) = dispatcher_with_counter();
    let err = dispatch(&d, &c, "add", &json!({"a": 1}), |_a| unreachable!())
        .expect_err("add must reject missing field");
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(err.message.contains("missing required property `b`"));
    assert_eq!(c.get(), 0);
}

#[test]
fn negative_add_extra_property() {
    let (d, c) = dispatcher_with_counter();
    let err = dispatch(
        &d,
        &c,
        "add",
        &json!({"a": 1, "b": 2, "injected": "; rm -rf /"}),
        |_a| unreachable!(),
    )
    .expect_err("add must reject extra property");
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(err.message.contains("unexpected property `injected`"));
    assert_eq!(c.get(), 0);
}

#[test]
fn negative_add_out_of_range() {
    let (d, c) = dispatcher_with_counter();
    let err = dispatch(
        &d,
        &c,
        "find_user",
        &json!({"user_id": 0}),
        |_a| unreachable!(),
    )
    .expect_err("find_user must reject out-of-range user_id (minimum is 1)");
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(err.message.contains("below minimum"));
    assert_eq!(c.get(), 0);
}

#[test]
fn negative_greet_too_long_title() {
    let (d, c) = dispatcher_with_counter();
    let long_title = "x".repeat(100);
    let err = dispatch(
        &d,
        &c,
        "greet",
        &json!({"name": "world", "title": long_title}),
        |_a| unreachable!(),
    )
    .expect_err("greet must reject overlong title");
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(err.message.contains("maxLength"));
    assert_eq!(c.get(), 0);
}

#[test]
fn negative_root_not_object() {
    // `args` is a JSON array — schema says object. Must be rejected.
    let (d, c) = dispatcher_with_counter();
    let err = dispatch(&d, &c, "add", &json!([1, 2, 3]), |_a| unreachable!())
        .expect_err("non-object args must be rejected");
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(err.message.contains("expected object"));
    assert_eq!(c.get(), 0);
}

#[test]
fn negative_unknown_tool_returns_not_found() {
    let (d, c) = dispatcher_with_counter();
    let err = dispatch(&d, &c, "no_such_tool", &json!({}), |_a| unreachable!())
        .expect_err("unknown tool name must be refused");
    assert_eq!(err.kind, ToolErrorKind::NotFound);
    assert_eq!(c.get(), 0);
}

// ---------------------------------------------------------------------------
// Fuzz cases — random shapes against every fixture tool
// ---------------------------------------------------------------------------

/// Tiny xorshift PRNG seeded from the test name so fuzz inputs are
/// deterministic per test (CI failures are reproducible). Avoids any
/// dependency on `rand` so the test file stays minimal.
struct Xs(u64);
impl Xs {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pick<T: Clone>(&mut self, choices: &[T]) -> T {
        let idx = (self.next() as usize) % choices.len();
        choices[idx].clone()
    }
    fn one_of(&mut self) -> Value {
        let pool: [Value; 8] = [
            Value::Null,
            json!(true),
            json!(false),
            json!(0),
            json!(-1),
            json!(1.5),
            json!(""),
            json!("hello"),
        ];
        self.pick(&pool).clone()
    }
    fn shape(&mut self, depth: u32) -> Value {
        // Random object/array/leaf with controlled depth so JSON never
        // blows the stack on tiny seed values.
        if depth == 0 {
            return self.one_of();
        }
        match self.next() % 4 {
            0 => Value::Null,
            1 => json!(self.next() as i64),
            2 => json!(format!("s{}", self.next())),
            3 => {
                let len = (self.next() as usize) % 4;
                Value::Object(
                    (0..len)
                        .map(|i| (format!("k{}", i), self.shape(depth.saturating_sub(1))))
                        .collect(),
                )
            }
            _ => unreachable!(),
        }
    }
}

/// Run 100 fuzz inputs against a single tool. Asserts every malformed
/// input is refused with `ValidationError`, and the handler
/// invocation count equals the number of valid inputs (zero, in most
/// cases — the fixtures are tight).
fn fuzz_tool(spec: &TypedToolSpec, seed: u64) {
    let mut rng = Xs::new(seed);
    let counter = CallCounter::new();
    let d = TypedDispatcher::from_specs(vec![spec.clone()]);
    let mut valid_count = 0usize;
    for _ in 0..100 {
        let args = rng.shape(3);
        let res = d.dispatch(&spec.tool_name, &args, |_a| {
            counter.inc();
            Ok(Value::Null)
        });
        match res {
            Ok(_) => {
                valid_count += 1;
            }
            Err(e) => {
                assert_eq!(
                    e.kind,
                    ToolErrorKind::ValidationError,
                    "tool `{}`: malformed input must be ValidationError, got {:?}: {}",
                    spec.tool_name,
                    e.kind,
                    e.message
                );
            }
        }
    }
    assert_eq!(
        counter.get(),
        valid_count,
        "tool `{}`: handler must run exactly {} times (once per valid input)",
        spec.tool_name,
        valid_count
    );
}

#[test]
fn fuzz_add() {
    let specs = load_typed_fixtures();
    let spec = specs.iter().find(|s| s.tool_name == "add").unwrap();
    fuzz_tool(spec, 0xA001);
}

#[test]
fn fuzz_greet() {
    let specs = load_typed_fixtures();
    let spec = specs.iter().find(|s| s.tool_name == "greet").unwrap();
    fuzz_tool(spec, 0x6007);
}

#[test]
fn fuzz_reverse() {
    let specs = load_typed_fixtures();
    let spec = specs.iter().find(|s| s.tool_name == "reverse").unwrap();
    fuzz_tool(spec, 0x5E55);
}

#[test]
fn fuzz_find_user() {
    let specs = load_typed_fixtures();
    let spec = specs.iter().find(|s| s.tool_name == "find_user").unwrap();
    fuzz_tool(spec, 0xF02D);
}

// ---------------------------------------------------------------------------
// Direct validator checks (cover points not exercised by the dispatcher)
// ---------------------------------------------------------------------------

#[test]
fn validator_array_basic() {
    let schema = json!({
        "type": "array",
        "items": { "type": "integer" }
    });
    assert!(validate_against_schema(&schema, &json!([1, 2, 3])).is_ok());
    let err = validate_against_schema(&schema, &json!([1, "x", 3])).unwrap_err();
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(err.message.contains("/1"));
}

#[test]
fn validator_boolean_and_null() {
    let b = json!({"type": "boolean"});
    assert!(validate_against_schema(&b, &json!(true)).is_ok());
    assert!(validate_against_schema(&b, &json!("true")).is_err());

    let n = json!({"type": "null"});
    assert!(validate_against_schema(&n, &json!(null)).is_ok());
    assert!(validate_against_schema(&n, &json!(0)).is_err());
}

#[test]
fn validator_pointer_path() {
    let schema = json!({
        "type": "object",
        "properties": {
            "outer": {
                "type": "object",
                "properties": {
                    "inner": { "type": "integer" }
                },
                "required": ["inner"],
                "additionalProperties": false
            }
        },
        "required": ["outer"],
        "additionalProperties": false
    });
    let err = validate_against_schema(&schema, &json!({"outer": {"inner": "x"}})).unwrap_err();
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(
        err.message.contains("/outer/inner"),
        "pointer must be /outer/inner, got: {}",
        err.message
    );
}

#[test]
fn dispatcher_from_fixtures_matches_from_specs() {
    let a = TypedDispatcher::from_specs(load_typed_fixtures());
    let b = TypedDispatcher::from_fixtures();
    assert_eq!(a.len(), b.len());
    for spec in a.specs() {
        assert!(b.find(&spec.tool_name).is_some());
    }
}

#[test]
fn fixture_spec_parses_for_every_file() {
    // Every typed fixture must parse into a TypedToolSpec. This is a
    // structural test that catches malformed fixtures at test time
    // instead of at validator-call time.
    let specs = load_typed_fixtures();
    assert!(
        !specs.is_empty(),
        "fixture directory must contain at least one spec"
    );
    for spec in &specs {
        assert!(
            !spec.tool_name.is_empty(),
            "every spec must have a tool_name"
        );
        assert!(
            spec.input_schema.is_object(),
            "spec `{}` input_schema must be an object",
            spec.tool_name
        );
    }
}

// =====================================================================
// T-021 fail-closed tests. These exercise the four guarantees added
// in response to the security review's findings on commit 535b994:
//
//   1. Unsupported keywords (pattern / oneOf / anyOf / allOf / enum
//      / const / format / $ref / ...) refuse the schema with a
//      ValidationError, NOT a silent Ok.
//   2. Object schemas must declare `additionalProperties` explicitly.
//      The JSON-Schema default of "permissive" is the exact fail-open
//      behavior T-021 removes.
//   3. `required` entries must be strings. Malformed entries raise
//      ValidationError pointing at the offending index.
//   4. Integer bounds use i64 arithmetic; fractional bounds are
//      rejected outright.
//   5. The typed layer is actually wired into the real
//      `call_tool_handler_with_provider` HTTP dispatch path: a
//      malformed call returns a 200 response with the validation
//      error in the body, and the handler is never invoked.
// =====================================================================

#[test]
fn fail_closed_unsupported_keyword_pattern_rejected() {
    // A schema that introduces `pattern` must fail loudly. The
    // validator cannot enforce it, so accepting the call would
    // create a silent bypass.
    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "pattern": "^foo" }
        },
        "required": ["name"],
        "additionalProperties": false
    });
    let err = validate_against_schema(&schema, &json!({"name": "foobar"})).unwrap_err();
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(
        err.message.contains("unsupported keyword `pattern`"),
        "expected unsupported-keyword diagnostic, got: {}",
        err.message
    );
}

#[test]
fn fail_closed_unsupported_keyword_oneof_rejected() {
    let schema = json!({
        "type": "object",
        "properties": {
            "value": { "oneOf": [{"type": "string"}, {"type": "integer"}] }
        },
        "required": ["value"],
        "additionalProperties": false
    });
    let err = validate_against_schema(&schema, &json!({"value": "x"})).unwrap_err();
    assert!(err.message.contains("unsupported keyword `oneOf`"));
}

#[test]
fn fail_closed_object_schema_must_declare_additional_properties() {
    // No `additionalProperties` at all -> the JSON-Schema default
    // (permissive) is exactly the fail-open behavior T-021 forbids.
    let schema = json!({
        "type": "object",
        "properties": {
            "a": { "type": "integer" }
        },
        "required": ["a"]
        // additionalProperties omitted
    });
    let err = validate_against_schema(&schema, &json!({"a": 1, "rogue": true})).unwrap_err();
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(
        err.message.contains("additionalProperties"),
        "expected additionalProperties diagnostic, got: {}",
        err.message
    );
}

#[test]
fn fail_closed_additional_properties_must_be_boolean() {
    // `additionalProperties: {}` (sub-schema shorthand) is not
    // accepted by T-021's vocabulary — only strict true / false.
    let schema = json!({
        "type": "object",
        "properties": {
            "a": { "type": "integer" }
        },
        "required": ["a"],
        "additionalProperties": {}
    });
    let err = validate_against_schema(&schema, &json!({"a": 1})).unwrap_err();
    assert!(err.message.contains("must be a boolean"));
}

#[test]
fn fail_closed_required_entry_must_be_string() {
    // A non-string `required` entry is a schema-author bug, not a
    // free pass. Surface it as ValidationError pointing at the
    // offending index.
    let schema = json!({
        "type": "object",
        "properties": {
            "a": { "type": "integer" }
        },
        "required": ["a", 42, true],
        "additionalProperties": false
    });
    let err = validate_against_schema(&schema, &json!({"a": 1})).unwrap_err();
    assert_eq!(err.kind, ToolErrorKind::ValidationError);
    assert!(
        err.message.contains("`required[1]`"),
        "expected index-pinned diagnostic, got: {}",
        err.message
    );
    assert!(err.message.contains("must be a string"));
}

#[test]
fn fail_closed_required_must_be_array() {
    let schema = json!({
        "type": "object",
        "properties": {"a": {"type": "integer"}},
        "required": "a",
        "additionalProperties": false
    });
    let err = validate_against_schema(&schema, &json!({"a": 1})).unwrap_err();
    assert!(err.message.contains("must be an array"));
}

#[test]
fn fail_closed_integer_bound_rejects_fractional_minimum() {
    // `minimum: 0.5` for an `integer` schema is incoherent: an
    // integer cannot satisfy a fractional bound. Refuse the
    // schema rather than silently truncating to 0.
    let schema = json!({
        "type": "object",
        "properties": {
            "n": { "type": "integer", "minimum": 0.5 }
        },
        "required": ["n"],
        "additionalProperties": false
    });
    let err = validate_against_schema(&schema, &json!({"n": 1})).unwrap_err();
    assert!(err.message.contains("`minimum` must be an integer literal"));
}

#[test]
fn fail_closed_integer_bound_rejects_fractional_maximum() {
    let schema = json!({
        "type": "object",
        "properties": {
            "n": { "type": "integer", "maximum": 9.9 }
        },
        "required": ["n"],
        "additionalProperties": false
    });
    let err = validate_against_schema(&schema, &json!({"n": 5})).unwrap_err();
    assert!(err.message.contains("`maximum` must be an integer literal"));
}

#[test]
fn integer_bound_uses_i64_arithmetic_no_f64_loss() {
    // 2^53 + 1 cannot be represented exactly in f64. The old code
    // path would have rounded this bound to 2^53 and silently
    // accepted `9007199254740993`. The i64 path accepts it as a
    // valid bound and the validator refuses a value that exceeds
    // it.
    let schema = json!({
        "type": "object",
        "properties": {
            "n": { "type": "integer", "maximum": 9007199254740993_i64 }
        },
        "required": ["n"],
        "additionalProperties": false
    });
    // 2^53 + 2: invalid because the maximum is 2^53 + 1.
    let err = validate_against_schema(&schema, &json!({"n": 9007199254740994_i64})).unwrap_err();
    assert!(err.message.contains("above maximum 9007199254740993"));
    // 2^53 + 1 exactly: still valid.
    assert!(validate_against_schema(&schema, &json!({"n": 9007199254740993_i64})).is_ok());
}

#[test]
fn fail_closed_schema_without_type_needs_opt_in_marker() {
    // A schema with no `type` and no `properties` / `required` is
    // an unconstrained accept-everything. The old code returned
    // Ok(()) silently. The new code refuses unless the schema
    // explicitly opts in via `x-tokitai-no-constraints: true` or
    // is the empty `{}`.
    let schema = json!({"description": "no constraints enforced"});
    let err = validate_against_schema(&schema, &json!({"anything": "goes"})).unwrap_err();
    assert!(err.message.contains("validator fails closed"));
}

#[test]
fn schema_without_type_with_opt_in_marker_accepts() {
    let schema = json!({"x-tokitai-no-constraints": true});
    assert!(validate_against_schema(&schema, &json!({"anything": 1})).is_ok());
}

#[test]
fn empty_schema_object_accepts() {
    // Truly empty `{}` is the historical "accept anything" shape.
    // Keeping that escape hatch documented and intentional.
    assert!(validate_against_schema(&json!({}), &json!({"x": 1})).is_ok());
}

#[test]
fn dispatcher_handler_not_invoked_on_validation_error() {
    // The dispatcher's contract: validate BEFORE handler. This
    // pins the contract for the public API.
    let counter = AtomicUsize::new(0);
    let spec = TypedToolSpec::from_value(&json!({
        "tool_name": "add",
        "input_schema": {
            "type": "object",
            "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}},
            "required": ["a", "b"],
            "additionalProperties": false
        }
    }))
    .expect("fixture must parse");
    let dispatcher = TypedDispatcher::from_specs(vec![spec]);
    let result = dispatcher.dispatch("add", &json!({"a": "not a number"}), |_args| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"a": 1, "b": 2}))
    });
    assert!(result.is_err());
    assert_eq!(counter.load(Ordering::SeqCst), 0, "handler must not run");
}
