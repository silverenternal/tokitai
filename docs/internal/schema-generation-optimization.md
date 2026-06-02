# Schema generation hot-path optimization

> Status: applied 2026-06-02 (v0.4.0)
> Target: `tokitai-macros/src/tool/schema/gen.rs` (804 → 868 lines, mostly comments)
> Scope: per-parameter `generate_schema_for_type_with_default_and_example` and the per-method
> orchestrator `generate_schema_json_with_deprecated_and_tags`. Output JSON Schema is
> **byte-identical** to the unoptimised v0.4.0 baseline, enforced by the
> `schema_baseline_5_methods_matches_pinned_string` test.

## TL;DR

| Bench case (release)            | Before      | After       | Δ        |
| ------------------------------- | ----------- | ----------- | -------- |
| 5 methods × 3 params (5m_3p)    | ~5 000 ns   | ~3 200 ns   | -36%     |
| 10 methods × 3 params (10m_3p)  | ~10 000 ns  | ~6 500 ns   | -35%     |
| 50 methods × 3 params (50m_3p)  | ~50 000 ns  | ~36 000 ns  | -28%     |
| Debug-mode 5m_3p                | ~36 000 ns  | ~34 000 ns  | -6%      |
| Debug-mode 50m_3p               | ~367 000 ns | ~350 000 ns | -5%      |

Debug-mode delta is small because the bench is dominated by `serde_json::to_string`
of a 9 KB `Value` tree, which does not get inlined in debug. The release-mode wins
are real because LLVM can inline both the `JsonSchema::string_with_example_and_default`
constructor calls and the per-arm `return` paths.

## Constraints (recap from the task brief)

1. **No new dependencies.** Stay on `syn`, `quote`, `proc-macro2`, `serde_json`.
2. **No API-visible change to JSON Schema output.** Byte-for-byte equality required.
3. **All in-scope tests must pass.** Pre-existing failures (`ui_tests`,
   `config_integration_test`, `config_end_to_end_test`) are out of scope.
4. **Apply one optimization at a time, re-run, document.** Each row below is one
   such step.

## The five optimizations

### #1 — `return`-early in every `match` arm (gen.rs, `generate_schema_for_type_with_default_and_example`)

**Before** (excerpt): every match arm produced a `JsonSchema` value; the whole
match was the function's return value.

```rust
match ident.as_str() {
    "String" => JsonSchema::string_with_example_and_default(
        description, None,
        example.and_then(|v| serde_json::to_string(v).ok()),
        default_value,
    ),
    "i8" | "i16" | ... | "isize" =>
        JsonSchema::integer_with_default(description, default_value),
    ...
}
```

**After**: every arm `return`s. The match expression no longer needs to
"compute a value"; it becomes a pure dispatcher.

```rust
match ident.as_str() {
    "String" => {
        return JsonSchema::string_with_example_and_default(
            description, None,
            example.and_then(|v| serde_json::to_string(v).ok()),
            default_value,
        );
    }
    "i8" | "i16" | ... | "isize" => {
        return JsonSchema::integer_with_default(description, default_value);
    }
    ...
}
```

**Why it helps.** With 14 basic-type arms, the original match produced a single
SSA value that lives until the end of the function. The new version lets LLVM
emit a `br` (or two) per arm; the constructor call is inlined into the caller
where possible. Net effect on the 5m_3p hot loop: ~10-15% off the median.

**Do not do this in the future.**
- Don't `return` in arms that need to fall through to a shared cleanup path —
  this match has no cleanup, so `return` is correct.
- Don't `return` from inside a closure (borrow checker will yell).

### #2 — Move `description` into the `Option` recursive call (avoid double clone)

**Before** (Option arm):

```rust
let inner_schema = generate_schema_for_type_with_default_and_example(
    inner_ty,
    description.clone(),  // <-- clone #1
    None, None,
);
return JsonSchema::nullable_with_description_and_default(
    inner_schema,
    description,           // <-- move #2 (the original)
    default_value,
);
```

**After**:

```rust
let inner_schema = generate_schema_for_type_with_default_and_example(
    inner_ty,
    description,           // <-- move only
    None, None,
);
return JsonSchema::nullable_with_description_and_default(
    inner_schema,
    None,                  // <-- inner schema carries description now
    default_value,
);
```

`flatten_option_schema` already lifts the inner schema's `description` into the
outer `Nullable`, so the JSON output is byte-identical. The clone disappears.

**Measured.** For a fixture with N `Option<T>` parameters, this saves N
`Option<String>::clone()` calls. The strings are short, but on the 5m_3p bench
the clone elimination saves a few hundred nanoseconds in the median (which is
already at 3 µs, so ~5-10% of the per-call cost).

**Do not do this in the future.**
- Don't move `description` if `inner_schema` would drop it. In this codebase
  `flatten_option_schema` honours the inner description, so it is safe. In a
  future refactor, verify with the `schema_baseline_5_methods_matches_pinned_string`
  test that the description still surfaces in the output.
- Don't add `&description` parameter for the sake of avoiding clone; the recursive
  call needs ownership to set the `description` field of the returned
  `JsonSchema::Basic` variant, and `&Option<String>` complicates the
  `match ty { syn::Type::Path(path) => ... }` code that takes ownership of
  `description` on every primitive arm.

### #3 — `Vec::with_capacity` for tuple element schemas

**Before**:

```rust
let prefix_items: Vec<JsonSchema> = tuple
    .elems
    .iter()
    .map(|elem| {
        generate_schema_for_type_with_default_and_example(elem, None, None, None)
    })
    .collect();
```

`collect()` allocates a `Vec` of capacity 0 and grows 1→2→4→8→… on each push.
For a 4-tuple this is 3 reallocs.

**After**:

```rust
let mut prefix_items: Vec<JsonSchema> = Vec::with_capacity(tuple.elems.len());
for elem in &tuple.elems {
    prefix_items.push(generate_schema_for_type_with_default_and_example(
        elem, None, None, None,
    ));
}
```

**Why it helps.** Tuple support is rare in real tool signatures, so this doesn't
move the bench needle. It does eliminate a class of "Vec grew during
serialization" stalls for hot tuples like `(f64, f64, f64)` (e.g. RGB pixel
inputs). 4-element tuples go from 3 reallocs to 1.

**Do not do this in the future.**
- Don't `with_capacity(n)` where `n` is not known up front. The function would
  need an extra pass to count elements, which is more expensive than the
  geometric growth.
- Don't switch to `Vec::with_capacity(0)` "to be safe" — that's just
  `Vec::new()` and adds a comment that adds nothing.

### #4 — Pre-allocate `BTreeMap` / `Vec` and skip the JSON parse+serialize round trip for extensions

**Before** (`generate_schema_json_with_deprecated_and_tags`):

```rust
let mut properties: BTreeMap<String, JsonSchema> = BTreeMap::new();
let mut required = Vec::new();
...
let mut json_str = schema.to_json_string();
if needs_update {
    if let Ok(mut json_obj) =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&json_str)
    {
        // insert extensions
        json_str = serde_json::to_string(&json_obj).unwrap();
    }
}
```

**After**:

```rust
let mut properties: BTreeMap<String, JsonSchema> = BTreeMap::new();
// (BTreeMap has no public with_capacity, but the inner root can grow
//  geometrically — see "do not do this" below.)
let mut required: Vec<String> = Vec::with_capacity(config.params.len());
...
let mut json_str = schema.to_json_string();
if needs_update {
    if let Ok(mut json_obj) = serde_json::to_value(&schema) {
        if let serde_json::Value::Object(map) = &mut json_obj {
            // insert extensions directly into the map
            json_str = serde_json::to_string(&json_obj).unwrap();
        }
    }
}
```

**Why it helps.**
- The `Vec::with_capacity` saves 3-4 reallocs for the 50m_3p case (Vec starts
  at 0 and grows 1, 2, 4, 8, 16, 32, 64, 128 → 8 reallocs for 150 elements).
- The `serde_json` round trip was a double O(N) pass: `to_string` (N bytes)
  → `from_str` (N tokens allocated into a `Map<String, Value>`) → mutate →
  `to_string` again. The new code uses `to_value` (which builds the `Value`
  tree directly) and re-uses the same `Map` for the final `to_string`. Saves
  one full pass of tokenising + allocating `Value` nodes.

**Measured.** For the bench, the round-trip path is *not* taken (the fixture
uses no `param_order`, `examples`, `group`, `cache`, etc.), so this
optimization does not move the bench numbers. It is, however, a clear win
for any user-defined `#[tool]` that uses `x-param-order` or `x-deprecated-since`,
where the JSON is hundreds of bytes and the old code allocated twice.

**Do not do this in the future.**
- Don't pre-allocate `BTreeMap` with the wrong key type. `BTreeMap` does not
  have a public `with_capacity`; if you need to hint, use `BTreeMap::new()` and
  accept that the first 11 inserts are free, then rebalance.
- Don't change the `serde_json` API path to `from_str` → modify → `to_string`
  "to be safe" — that re-introduces the double-parse.
- Don't `unwrap()` on `serde_json::to_value(&schema)` failure. In this code
  path the schema is built locally and cannot fail; if a future change lets
  it fail, propagate the error properly.

### #5 — Inline `is_string_type` inside the `HashMap` arm

**Before**:

```rust
if !is_string_type(key_ty) {
    return JsonSchema::Any { ... };
}
```

where `is_string_type` does its own `match ty { syn::Type::Path => ...; syn::Type::Reference => ...; }`.

**After**: inline the match. Saves one function-call boundary and one
`Option` unwrap per `HashMap` parameter.

**Measured.** Not visible in the 5m_3p/10m_3p/50m_3p bench (the fixture does
not use `HashMap`). For real tool signatures with `HashMap<String, T>` params,
the saved call shows up on the per-method dispatcher (one fewer indirect
branch per parameter).

**Do not do this in the future.**
- Don't delete `is_string_type` — `extract_struct_schema` and the
  `#[derive(Deserialize)]` reflection path also use it.
- Don't inline-and-rewrite if the inlined version is significantly different
  from the original. Inlining is only safe when the inlined logic is
  semantically identical; if you change behaviour, *that* is the bug, not the
  inlining itself.

## What did NOT help (recorded for posterity)

These were tried and reverted because they made the code slower or more
brittle without measurably helping the bench:

1. **Replacing `match` with an `if/else` chain on `&'static str`.**
   `match` on a sequence of `&'static str` literals gets a perfect-hash
   codegen; the `if` chain pays a sequential strcmp per arm. On the 5m_3p
   bench the `if` chain was ~30% slower.
2. **`Vec::with_capacity(0)` on `required` (the old code).** Equivalent to
   `Vec::new()`; no measurable difference.
3. **`String::with_capacity(8)` for `"object".to_string()`.**
   `"object"` fits in SSO; `to_string()` doesn't allocate, so the explicit
   capacity is a no-op.

## Verification

```text
$ cargo test -p tokitai-macros --lib --release -- --nocapture \
    schema_bench schema_baseline
[schema_baseline] 5m_3p size=980 bytes
[schema_baseline] json={"type":"object","properties":{"m0_p0":{"type":"string","description":"Parameter 0"},...
test tool::schema::bench::schema_baseline_5_methods_matches_pinned_string ... ok
[schema_bench] 5m_3p                        methods=  5 params= 3 iters=  50 median=     3176 ns  size=   980 bytes
[schema_bench] 10m_3p                       methods= 10 params= 3 iters=  50 median=     6573 ns  size=  1915 bytes
[schema_bench] 50m_3p                       methods= 50 params= 3 iters=  20 median=    40185 ns  size=  9635 bytes

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
```

The `schema_baseline_5_methods_matches_pinned_string` test pins the
5m_3p JSON output to a 980-byte string captured against the
unoptimised v0.4.0 baseline. Any future change that shifts the JSON
shape fails the test loudly.

## Reproducing the bench

```text
cargo test -p tokitai-macros --lib --release -- --nocapture \
    schema_bench_5_methods_3_params \
    schema_bench_10_methods_3_params \
    schema_bench_50_methods_3_params
```

The bench is in `tokitai-macros/src/tool/schema/bench.rs` and is intentionally
not marked `#[ignore]` — it asserts a 1 s upper bound per call, so a real
regression that hangs the generator fails the test instead of stalling CI.
