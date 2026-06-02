# Property-Based Testing for the `#[tool]` Proc-Macro

**Date:** 2026-06-02
**Scope:** `tokitai-macros/tests/property_based_test.rs`
**Author:** QA pass

---

## 1. Why property-based tests?

The `#[tool]` proc-macro is a large, branching piece of code whose
input space is the set of *all* syntactically valid Rust `impl`
blocks. Hand-written fixtures (e.g. `golden_output_test.rs`,
`compile_time_optimization_test.rs`) cover the *common* shapes
thoroughly but cannot easily reach the corners of that input space
— the very long method name, the deeply nested `Option<Vec<…>>`
parameter, the mix of `&self` and `&mut self`, the empty
description, the parameter whose type is a custom user struct.

Property-based testing (via `proptest`) lets us state *invariants*
about the macro pipeline and have the framework generate hundreds
of small, randomly-shrunk inputs that falsify them.

---

## 2. The bridging problem

The `#[tool]` proc-macro is **compile-time** code: it runs when
`cargo build` consumes the user's `impl` block and turns it into
Rust source. `proptest`, by contrast, is a **runtime** testing
framework. Naively, you cannot put a runtime-generated `impl`
block into a proc-macro.

The test file solves this with two complementary bridges:

### 2.1 Hidden compile-time proc-macros

Two `#[proc_macro]` hooks live in `tokitai-macros/src/lib.rs`
next to the existing `__bench_expand_tool` benchmark hook:

```rust
#[proc_macro] pub fn __property_expand(item: TokenStream) -> TokenStream
#[proc_macro] pub fn __property_would_error(item: TokenStream) -> TokenStream
```

- `__property_expand` takes an impl block as a `TokenStream`,
  runs it through the internal `tool::tool` pipeline, and emits
  a `&'static str` literal containing the rendered expansion.
- `__property_would_error` does the same but emits a `bool`
  literal — `true` if the expansion contains a `compile_error!`
  invocation, `false` otherwise.

The proptest file uses these to assert that the **real** macro
produces a stable, well-formed expansion for a small, fixed set
of impl blocks (one 5-method all-sync fixture, one reserved-name
violation, one happy-path fixture). The expansions of these are
checked against a hand-written snapshot in
`tests/fixtures/property_based_snapshot.txt`.

### 2.2 Runtime replica of the validation pipeline

For the *shrinking-friendly* exploration, the test file
re-implements a syn-based replica of the macro's parsing and
validation logic locally:

- `is_tool_method(fn_item) -> bool` — same predicate the macro
  uses to decide whether a method becomes a tool.
- `runtime_reject_reason(method) -> Option<&'static str>` —
  same error codes the macro's validator emits.
- `run_runtime_pipeline(src) -> PipelineOutcome` — parses an
  impl-block string with `syn::parse_str`, runs every method
  through the validator replica, and returns the verdict.

This is **deliberately a replica**, not the real validator, for
two reasons:

1. The real `validate_tool_method` lives in a `pub(crate)`
   module — exposing it just for tests would be a public-API
   change.
2. The replica is a *strict subset* of the real validator's
   rules. If proptest finds a violation in the replica, it
   strongly suggests a real violation in the macro; if it
   doesn't, the macro is at least as safe as the replica on
   the explored input space.

The proptest strategies generate impl-block **strings** at
runtime, the runtime replica parses them, and the invariants
fire.

---

## 3. Input generation strategies

The strategies are in `tokitai-macros/tests/property_based_test.rs`
and cover two halves of the input space:

### 3.1 `arb_valid_impl()`

Generates impl blocks that **should** be accepted by the
validator:

- 1–10 methods per impl block.
- Method names drawn from a fixed pool of lowercase identifiers
  (`"add"`, `"sub"`, `"mul"`, `"search"`, `"display_name"`, ...).
- Receivers restricted to `&self`.
- Parameter types drawn from a pool of `i32`, `String`,
  `Option<bool>`, `Option<Vec<i64>>`, `Option<String>`.
- Optional doc comments, optional `#[allow(dead_code)]`.

### 3.2 `arb_invalid_impl()`

Generates impl blocks that **should** be rejected:

- Method returning `Self`.
- Method with no `self` receiver.
- Generic method.
- `async fn` with `&mut self`.
- Method whose name starts with `__` (reserved).
- Method whose name is `call_tool` / `tool_definitions` /
  `configure_tool` (would shadow the macro's injected items).

The `arb_violation()` strategy is a tagged enum so that a
single input can carry several violations at once; proptest
shrinks to the smallest still-violating input.

### 3.3 Sanity strategy

`sanity_proptest_works` uses `(x in 0u32..100)` to assert
`x + 0 == x`. This is a smoke test for the proptest machinery
itself — if this ever fails, the rest of the file's proptest
blocks are suspect.

---

## 4. Invariants under test

The file contains **11** tests in total: 6 inside `proptest!`
blocks and 5 outside.

| Test name | Type | Property |
|-----------|------|----------|
| `sanity_proptest_works` | proptest (10 cases) | proptest machinery is wired up |
| `runtime_pipeline_does_not_panic_on_valid_impls` | proptest (50 cases) | parser+validator do not panic on well-formed inputs |
| `macro_rejects_every_violation_in_pool` | proptest (50 cases) | every `arb_violation()` produces a non-`None` reject reason |
| `pipeline_is_deterministic` | proptest (50 cases) | same input string yields byte-identical pipeline output |
| `pipeline_ignores_method_order` | proptest (50 cases) | method reordering does not change the schema string (modulo whitespace) |
| `runtime_rejection_is_bounded` | proptest (50 cases) | reject reasons are drawn from a finite enum, never `unreachable!()` |
| `compile_time_would_error_for_invalid_impls` | unit | real macro emits `compile_error!` for a reserved-name + missing-`self` impl |
| `compile_time_would_not_error_for_valid_impl` | unit | real macro does **not** emit `compile_error!` for a happy-path impl |
| `compile_time_expansion_is_stable` | unit | real macro's rendered expansion is non-empty and contains the expected `__call_<name>` shims |
| `runtime_pipeline_agrees_with_real_macro` | unit | the runtime replica and the real macro agree on a small shared fixture |
| `snapshot_5_method_fixture` | trybuild-style snapshot | the real macro's expansion of a 5-method fixture matches `tests/fixtures/property_based_snapshot.txt` |

The proptest blocks each use
`ProptestConfig::with_cases(50)` (10 for the sanity block) to
keep total runtime under a few hundred milliseconds; bump it
locally with `PROPTEST_CASES=512 cargo test ...` if you want
more aggressive exploration.

**Total proptest cases per default run:**
10 + 50 + 50 + 50 + 50 + 50 = **260**.

---

## 5. Snapshot re-baselining

`tests/fixtures/property_based_snapshot.txt` holds the
expected expansion of the 5-method fixture. To re-baseline
after an intentional macro change:

```sh
BLESS=1 cargo test -p tokitai-macros --test property_based_test snapshot_5_method_fixture
# or, equivalently:
TOKITAI_BLESS=1 cargo test -p tokitai-macros --test property_based_test snapshot_5_method_fixture
```

If the snapshot file does not exist at all (a fresh
checkout), the test auto-creates it on first run. This avoids
the bootstrapping problem where a new contributor cannot run
the test without an external `BLESS` step.

The snapshot test normalizes whitespace before comparison, so
a benign `quote!` whitespace tweak does not break it.

---

## 6. Extending the test set

To add a new property:

1. **Add a strategy** in the `Strategies` section. Keep the
   strategy small and focused — a 50-case proptest with a
   100-element input space is more valuable than a
   10,000-element one with a 10,000,000-element space,
   because proptest's shrinking will find the *smallest*
   failing input regardless.
2. **Add a `proptest!` block** with a clear one-line property
   statement. Prefer `prop_assert!` / `prop_assert_eq!` over
   `assert!` / `assert_eq!` so the failure is reported with
   the shrunk input.
3. **Avoid `prop_assume!`** where possible. `prop_assume!` is
   a silent filter — it discards inputs without telling you
   why. Use a tighter strategy (e.g. `prop_filter_map` or
   `prop::collection::vec(0..10, ...)`) so that rejected inputs
   are visible in the proptest statistics.
4. **For compile-time invariants**, prefer a dedicated
   `#[test]` using the `__property_expand` /
   `__property_would_error` hidden proc-macros over running
   the macro in a `trybuild` fixture. The hidden proc-macros
   turn the expansion into a `&'static str`, which is easier
   to compare against a snapshot than a full Rust file.

---

## 7. Coverage and gaps

The current property-based suite covers:

- Parser-validator pipeline (no-panic, determinism, order
  independence, rejection reason enum).
- Compile-time macro expansion (snapshot of a 5-method
  fixture, presence of `__call_<name>` shims).
- Macro-level rejection (`compile_error!` emission for
  reserved-name and missing-`self` violations).

It does **not** cover:

- **Async / sync mixing.** `arb_valid_impl` is all-sync by
  default. Async variants are covered by the existing
  `mixed_sync_async_test.rs` and `async_sync_interop_test.rs`
  hand-written fixtures.
- **Generic methods with where-clauses.** Proptest
  shrinking is much more useful on flat combinators; the
  macro's generic-method error path is exercised by the
  hand-written `ui/errors/` trybuild fixtures.
- **OpenAPI and wrap modes.** Those have their own
  dedicated test files (`wrap_openapi_test.rs`,
  `wrap_native_test.rs`).
- **Compile-time metric invariants** (token count, file size
  of the generated source). Those live in
  `compile_time_optimization_test.rs` because they need
  access to the build's `cargo:rustc-link-arg` machinery.

---

## 8. Reference

- `tokitai-macros/tests/property_based_test.rs` — the test
  file itself.
- `tokitai-macros/tests/fixtures/property_based_snapshot.txt`
  — the pinned 5-method expansion.
- `tokitai-macros/src/lib.rs` — the two hidden proc-macro
  hooks (`__property_expand`, `__property_would_error`),
  located next to `__bench_expand_tool`.
- `proptest` 1.5 — the property-based testing framework. See
  <https://proptest-rs.github.io/proptest/intro.html> for an
  overview.
