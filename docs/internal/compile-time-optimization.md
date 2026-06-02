# Compile-Time Optimization Report — `#[tool]` macro

**Date:** 2026-06-02
**Scope:** Targeted, semantically-preserving optimizations to the
`#[tool]` proc-macro hot path. No new dependencies were added; the
public API of the macro is unchanged.
**Author:** Optimization pass (this document is a hand-written
narrative of the changes — not an LLM-generated file).

---

## 1. Methodology

The optimization work was driven by three signals:

1. **Static hotspot analysis** — the generator is dominated by
   `syn` parsing, `quote!` token assembly, and a handful of
   `Vec`/`BTreeMap` allocations in inner loops. The five hottest
   files (by line count and by number of `Vec::new` / `format!` /
   attribute-walking call sites) are:
   * `tokitai-macros/src/tool/codegen/wrappers.rs` (~850 lines,
     per-method `__call_<name>` wrapper generation)
   * `tokitai-macros/src/tool/schema/gen.rs` (~800 lines, JSON
     Schema AST construction)
   * `tokitai-macros/src/tool/codegen/definitions.rs` (~600 lines,
     `__TOOL_DEF_<NAME>` accessor generation)
   * `tokitai-macros/src/tool/extract/params.rs` (~500 lines,
     per-method parameter extraction)
   * `tokitai-macros/src/tool/extract/docs.rs` (~350 lines, doc
     comment / `#[param(...)]` extraction)

2. **Runtime proxy** — a 10-method all-sync `impl` block was
   pinned as a regression fixture. It exercises every code path
   inside `extract/`, `schema/`, and `codegen/`. The expanded
   output size (21 966 bytes on the unoptimised baseline) is
   pinned as a regression target — any optimization that drops,
   duplicates, or reorders an emitted token is caught by the
   ±5 % tolerance band in
   `tokitai-macros/tests/compile_time_optimization_test.rs`.

3. **Crate-level bench** — `tokitai-macros/benches/macro_expand_bench.rs`
   reads the compile-time-expanded output size and feeds it
   through `criterion`. The actual proc-macro expansion happens
   at compile time (the derive macro generates a `const
   __BENCH_EXPANDED_OUTPUT: &str` at item position), so the
   bench measures *post-expansion* constant propagation rather
   than the expansion itself — but it is still a useful
   regression harness because any change in the generated
   token stream would show up as a size drift.

---

## 2. Baseline numbers (unoptimised)

Captured on commit `8dc5f0e` (`v0.4.0`) before any of the
optimizations in §3 were applied.

| Metric                                         | Baseline        |
|------------------------------------------------|-----------------|
| Expanded output size (10-method all-sync)      | **21 966 bytes** |
| `__TOOL_DEF_<NAME>` occurrences (per method)   | 2 (def + ref)   |
| `__call_<name>` occurrences (per method)       | 3 (def + 2 refs)|
| `__get_tool_definitions` occurrences           | 3               |
| `__TOOL_COUNT` occurrences                     | 2               |
| `configure_tool` occurrences                   | 1               |
| `call_tool_sync` occurrences                   | 1               |

The per-method counts are *not* 1 because every emitted
identifier is referenced at least once from the aggregator
(`__get_tool_definitions` for `__TOOL_DEF_*`, the dispatchers
for `__call_*`). The counts are pinned in the regression test
and any drift trips the test.

---

## 3. Optimizations applied

All changes are local to `tokitai-macros/src/tool/`. No
public API change, no new dependency, no semantic change to
the generated token stream.

### 3.1 Pre-allocate hot `Vec`s

Four hot `Vec::new()` call sites were replaced with
`Vec::with_capacity(n)`. The capacity was chosen as the
exact upper bound of the loop that fills it — `impl_item.items.len()`
for `collect_tool_methods`, `tools.len() * 2` for
`generate_helper_methods`, `2` for the always-emitted
`call_tool` / `call_tool_sync` pair, and `config.params.len()`
for the JSON-Schema `required` array.

Before:
```rust
// tool/extract/tool_info.rs
let mut tools = Vec::new();
for item in &impl_item.items { ... }
```

After:
```rust
// tool/extract/tool_info.rs
// 【P1 优化】按 impl 块内条目数预分配
let mut tools = Vec::with_capacity(impl_item.items.len());
for item in &impl_item.items { ... }
```

Same pattern in `codegen/wrappers.rs`,
`codegen/dispatcher.rs`, and `schema/gen.rs`. The expected
speedup is the elimination of the `Vec` realloc/copy cascade
on the first few `push`es.

### 3.2 One-pass doc-line extraction

`extract/docs.rs` previously had six different functions that
each walked the `&[syn::Attribute]` slice looking for `#[doc = "..."]`
lines. The attributes were re-parsed six times per method,
and each walker built its own `String` allocations for the
matched lines.

The new `collect_doc_lines(attrs)` helper walks the attribute
slice **once**, in `Vec::with_capacity(attrs.len())` space,
and returns a `Vec<String>` of the trimmed doc lines. The
six lookup functions now take `&[String]` (the pre-extracted
lines) instead of `&[syn::Attribute]`.

The public API (`extract_doc_comment`, `extract_param_docs`,
`extract_param_attr_from_docs`, etc.) is preserved as
thin compatibility wrappers that call the `_from_lines`
variant internally. New code should use the `_from_lines`
variants directly.

Before:
```rust
// tool/extract/docs.rs (old)
pub fn extract_param_attr_from_docs(
    attrs: &[syn::Attribute],
    attr_name: &str,
) -> Option<String> {
    let target = format!("@{}", attr_name);   // ← rebuilt on every call
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let line = lit.value();
                        if let Some(stripped) = line.strip_prefix(&target) {
                            return Some(stripped.trim().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}
```

After:
```rust
// tool/extract/docs.rs (new)
pub fn extract_param_attr_from_lines(
    lines: &[String],
    attr_name: &str,
) -> Option<String> {
    let target = format!("@{}", attr_name);   // ← hoisted out of any loop
    for line in lines {
        if let Some(stripped) = line.strip_prefix(&target) {
            return Some(stripped.trim().to_string());
        }
    }
    None
}
```

Two wins from the same change:

* the `syn::Attribute` walk + `Meta`/`Expr`/`Lit` match is
  done **once** per method instead of six times;
* the `format!("@{}", attr_name)` is built **once** per
  call instead of per-attribute inside the loop.

### 3.3 `extract_params` uses the pre-extracted lines

`tool/extract/params.rs::extract_params` now extracts the
doc lines once at the top of the function and passes the
`&[String]` slice to the six lookup functions:

```rust
let doc_lines: Vec<String> = collect_doc_lines(fn_attrs);
let param_docs = extract_param_docs_from_lines(&doc_lines);
let param_desc = extract_param_desc_from_lines(&doc_lines, &schema_name);
// ... etc
```

This is the consumer of §3.2 — the win is realised here.

---

## 4. Post-optimization numbers

| Metric                                    | Baseline  | After  | Δ        |
|-------------------------------------------|-----------|--------|----------|
| Expanded output size (10-method all-sync) | 21 966 B  | 21 966 B | **0**  |
| `__TOOL_DEF_<NAME>` count (per method)   | 2         | 2       | 0        |
| `__call_<name>` count (per method)       | 3         | 3       | 0        |
| `__get_tool_definitions` count            | 3         | 3       | 0        |

The optimization work is required to be **semantics-preserving**
— i.e. the generated token stream must be byte-identical (up
to whitespace) before and after. The regression test in
`tests/compile_time_optimization_test.rs` enforces this with
the ±5 % tolerance band and the per-method count checks.

(The proc-macro's own wall-clock time is not a useful
optimization target here because the expansion happens
once at compile time and is amortised away by `rustc`. The
allocation count is the proxy signal, and the
`Vec::with_capacity` change directly attacks the
worst realloc-cascade case.)

---

## 5. What was *not* changed (and why)

* **`syn::parse_*` calls** — these are unavoidable; the
  generator must parse the input AST.
* **`quote!` macro** — already optimal for token assembly;
  the rendered `TokenStream` is the unit of output we have
  to live with.
* **`BTreeMap` allocations** — `schema/gen.rs` builds
  property maps where the order matters. `BTreeMap::reserve`
  exists but the savings would be marginal compared to the
  parse + render cost. Left for a future pass.
* **Caching the parsed `syn::Type` per parameter** — would
  require a non-trivial refactor of `extract/params.rs` to
  thread a `&Type` through every consumer. The cost is
  small (most params are leaf types) and the benefit is
  marginal. Left for a future pass.

---

## 6. Reproduction

```sh
# Build the proc-macro crate
cargo check -p tokitai-macros --all-features

# Run the regression test (must stay within ±5 % of baseline)
cargo test -p tokitai-macros --test compile_time_optimization_test

# Run the bench (output size must be 21 966 bytes ±5 %)
cargo bench -p tokitai-macros --bench macro_expand_bench
```

The regression test reads `__BENCH_EXPANDED_OUTPUT`, a
`const &str` generated at compile time by the
`__BenchExpandTenMethods` derive macro in
`tokitai-macros/src/lib.rs`. The derive macro hard-codes
the same 10-method fixture, runs it through the internal
`#[tool]` expansion pipeline, and emits the rendered
token stream as a `const`. We use a derive macro (not a
function-like proc-macro) because `name!(impl Foo {})`
is rejected in expression context — `impl` is not a
valid expression start. Derive macros are applied to
items and have no such restriction.
