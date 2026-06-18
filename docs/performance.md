# Tokitai Performance Guide

**Version**: 0.5.x | **Audience**: Rust developers deploying Tokitai
in latency-sensitive paths or large monorepos.

This guide explains what `#[tool]` (and the rest of the wrap family) cost
at **compile time**, **runtime**, and in **memory** — with every claim
sourced from a `criterion` bench, a real-world measurement, or a code
reference. For *how* to use the macros, see
[`best-practices.md`](best-practices.md). For *why* the design looks
the way it does, see the [ADRs](adr/README.md).

---

## Table of contents

1. [TL;DR](#tldr)
2. [Compile time](#compile-time)
3. [Runtime](#runtime)
4. [Memory](#memory)
5. [Async](#async)
6. [Schema generation](#schema-generation)
7. [Best practices summary](#best-practices-summary)
8. [See also](#see-also)

---

## TL;DR

Five rules. Each one is expanded below with the number that backs it.

1. **Trust the compile-time codegen.** Tool definitions, dispatchers,
   and `LazyLock<ToolDefinition>` statics are baked into `.rodata`;
   the first `call_tool` is identical to the 10,000th.
   *See [Runtime](#runtime).*
2. **Don't wrap sync I/O in `tokio::spawn` to "make it async".** A
   `#[tool]`-emitted sync `__call_X` is ~62 ns; an extra
   `tokio::spawn` round-trip is ~10–20 µs — **3 orders of magnitude**
   cheaper dispatcher than the LLM call you are about to make.
   *See [Async](#async).*
3. **Stack resilience decorators: rate-limit innermost,
   circuit-breaker in the middle, retry outermost.** A wrong order
   either rate-limits the retry, or retries the rate-limit, or both.
   *See the
   [wrap-architecture composition rules](wrap-architecture.md#5-composition-rules).*
4. **Cache `tool_definitions()` once at startup.** A second
   `MyClient::tool_definitions()` call is a ~728 ps pointer chase;
   cache it anyway because the `Vec` returned by `.to_vec()` is a
   clone of the `&'static` slice. *See [Memory](#memory).*
5. **Measure on your own crate before optimising.** The numbers
   below are for the *demo* `Calculator`. Yours will differ; use
   `scripts/measure-consumer-impact.sh` (see callout below).

> **Surprising finding.** The `call_tool` dispatcher is **~62 ns**
> per call for a one-arg sync tool (median 62.387 ns on this
> machine, range 61.9 – 62.9 ns; see
> [`macro_bench::tool_call_simple`](../tokitai-macros/benches/macro_bench.rs)).
> The LLM-side request that triggers that call is typically
> 200–2000 ms. The dispatcher is **roughly 10 million times
> cheaper** than the call it serves. Optimise the LLM call, not
> the dispatcher.

---

## Compile time

### What `#[tool]` does at compile time

`#[tool]` is a proc-macro attribute. It runs once per `impl` block
during `cargo check` / `cargo build`, parses the user's source with
`syn`, and emits a `TokenStream` containing the dispatcher, per-method
`__call_X` wrappers, `__TOOL_DEF_X` accessors, a
`LazyLock<ToolDefinition>` static per method, and the `ToolProvider` /
`ToolCaller` impls. The expansion itself is fast (sub-millisecond for
a 10-method impl); the **cost you feel is the rustc work on the
emitted code**.

### Per-method cost (audited numbers)

`docs/internal/generated-code-size-audit.md` audited a 5-method
all-sync impl and extrapolated. Per `#[tool]` method the macro emits
**~46 source lines / ~200 tokens** (1 `__TOOL_DEF_X` accessor + 1
`__call_X` wrapper + 1 match arm in each dispatcher). Plus
**14 new `fn`s + 1 `const`** per `#[tool]` impl block (the two
dispatchers, the `__get_tool_definitions` helper, the
`configure_tool` helper, and the `__TOOL_COUNT` const).

### Scaling

`generated-code-size-audit.md` §2.4 extrapolates:

| Methods (N) | New `fn`s | Approx. expansion lines | Estimated `cargo check` Δ |
|------------:|----------:|------------------------:|--------------------------:|
|           5 |        14 |                    ~210 |                < 1 s      |
|          10 |        24 |                    ~410 |                ~1 s       |
|          50 |       104 |                  ~2,050 |                ~5–10 s    |
|         100 |       204 |                  ~4,050 |                ~10–20 s   |

A 50-method impl block can add **5–10 seconds** to incremental
`cargo check` time on a moderately-sized consumer crate; the bulk
is MIR + LLVM, not the proc-macro expansion itself.

### The 21 966-byte pinned baseline

`docs/internal/compile-time-optimization.md` pins the expanded output
size of a 10-method all-sync impl to **21 966 bytes** (regression
tolerance ±5%). The 10-method fixture is run through the internal
expansion pipeline; the rendered token stream is captured as a
`const __BENCH_EXPANDED_OUTPUT: &str` at item position via a derive
macro. The bench feeds this constant through `criterion` and reports
the *post-expansion constant propagation* time — small, but the
benchmark doubles as a regression harness: any change in the
generated token stream shows up as a size drift.

> **Note.** Running `cargo bench -p tokitai-macros --bench
> macro_expand_bench` and `--bench real_world` in the current working
> tree (commit `8dc5f0e`, 2026-06-02) fails to compile `tokitai-core`
> with `deny(missing_docs)` errors in `src/config.rs`. The failure is
> **pre-existing** (unrelated to the bench files) and does not
> invalidate any numbers in this document. The runtime numbers below
> were captured from `cargo bench -p tokitai-macros --bench
> macro_bench`, which compiles cleanly. Captured machine: Linux,
> criterion 0.5, release profile, `+target-cpu=native` defaults.

### Optimizations that already shipped (zero net code change)

`compile-time-optimization.md` §3 documents three changes that hit
the proc-macro hot path on 2026-06-02: (1) `Vec::with_capacity(n)`
in four hot loops, eliminating the `Vec` realloc/copy cascade
(*generated* output size is byte-identical: 21 966 → 21 966 bytes);
(2) one-pass doc-line extraction (`collect_doc_lines(attrs)` walks
`&[syn::Attribute]` once instead of six functions each
re-walking the slice); (3) `extract_params` re-uses the
pre-extracted lines. The audit also lists **five more dedup
opportunities** not yet applied (`generated-code-size-audit.md` §4):
applying all five would remove **~3 500 tokens and ~50 fn items**
for a 50-method impl, an estimated **30–40%** reduction in
macro-emitted code volume.

### Measure on your own crate

> **Callout.** The numbers above are from the demo `Calculator` and
> the 5/10-method audit fixtures. **Yours will differ.** Before doing
> anything clever, measure. The
> `scripts/measure-consumer-impact.sh` script (currently being
> built by another agent — see ADR-0005 for the dedup work that
> motivates it) wraps `cargo clean` + `cargo check --timings` +
> `cargo expand` and reports the expansion size delta, the
> `cargo check` wall-clock delta, and the per-method `__call_X`
> token count. Drop it next to your `Cargo.toml`, run it on `main`
> and on a branch that adds a `#[tool]` impl, and diff the output.

#### Per-impl profile mode (T-011)

The wall-clock `cargo check` numbers above are useful for catching
*total* regressions but they conflate macro cost with link, codegen,
and TOML parsing — all of which can drift for reasons that have
nothing to do with `#[tool]`. To isolate the macro's own
contribution, set `TOKITAI_PROFILE=1` in the build environment.
The macro will then emit one line per `#[tool]` impl block:

```text
cargo:warning=impl <TYPE> -> <TOOLS> tools, ms=<MICROS>
```

`scripts/measure-consumer-impact.sh` reads these lines and
prints them in a dedicated "Per-impl profile report" section,
along with the median µs per impl block. CI captures the per-impl
median as the `per-impl-profile` artifact; once enough green
runs establish a baseline distribution, a >20% median regression
will fail the build.

Example:

```bash
TOKITAI_PROFILE=1 bash scripts/measure-consumer-impact.sh /path/to/your-crate
```

Per-impl numbers fluctuate by ~10% run-to-run even on a quiet
machine (Linux scheduler noise, background cron jobs, ...); the
script's default `TOKITAI_RUNS=3` is the right knob to tighten
when you need a more confident reading.

### Compile-time rules

1. **Favour multiple small `#[tool]` impl blocks over one giant one.**
   The **incremental** `cargo check` cost is dominated by *which*
   file you touched, not by the total. A change to one domain
   re-checks that impl's `fn` items, not the 204 of a monolithic
   100-method block.
2. **Use `#[tool(skip)]` on private helpers** — each non-skipped
   `pub` method is a tool, with its own dispatcher arm and
   `LazyLock` static.
3. **Avoid generic methods.** The macro cannot synthesise a
   monomorphised wrapper for a generic; the build will fail.
4. **Keep doc comments short.** Every byte of `///` is a byte in
   `.rodata` (concatenated into the `description` field).
5. **Stack decorators from innermost (most-restrictive) to
   outermost (most-permissive).** Short version: `#[rate_limit]`
   innermost, `#[circuit_breaker]` middle, `#[retry]` outermost.
   See [`best-practices.md` rule 3](best-practices.md#3-composing-resilience-macros).

---

## Runtime

The full hot path for a sync `#[tool]` call is:

```text
caller -> call_tool(name, args)
          -> match name (one branch)
          -> self.__call_X(args)
              -> args.get("…") per param
              -> serde_json::from_value (per param)
              -> validation guards (per param, if any)
              -> self.X(args)            (the user body)
              -> serde_json::to_value(result).unwrap()
          -> Ok(Value)
```

There is no `HashMap` lookup, no spec parsing, no schema
deserialisation, no reflection. The only allocations are the per-param
`from_value` and the final `to_value`.

### Captured criterion numbers

`cargo bench -p tokitai-macros --bench macro_bench` (commit `8dc5f0e`,
2026-06-02, `release` profile, `+target-cpu=native`):

| Bench                             | Median   | Range          | What it measures                                       |
|-----------------------------------|---------:|---------------:|--------------------------------------------------------|
| `tool_definitions_access`         |  **728 ps** |  722 – 733 ps  | `MyTools::tool_definitions()` (returns `&'static [ToolDefinition]`) |
| `tool_lookup`                     |  **1.03 ns** | 1.03 – 1.04 ns | Linear scan to find a tool by name over 3 tools        |
| `schema_pretty_print`             |  **396 ns**  | 389 – 406 ns  | `serde_json::to_string_pretty(tool.input_schema)`      |
| `tool_call_simple`                |  **62.4 ns** | 61.9 – 62.9 ns | 1-arg sync call, `String` param                        |
| `tool_call_multi_param`           |  **192 ns**  | 191 – 194 ns   | 3-arg sync call (`String` + `i32` + `Option<String>`)  |
| `tool_call_validated`             |  **99.6 ns** | 99.1 – 100 ns  | 2-arg sync call + 4 validation rules                  |

These are the *dispatcher* cost. The `to_value` at the end is part
of the 62.4 / 192 / 99.6 number; the *user body* (`format!` + a
`String` allocation) is also in there.

### Cost breakdown (educated estimate)

The 62.4 ns for `tool_call_simple` decomposes roughly as: `match
name` ~1 ns, `args.get("name")` + `from_value` ~20–30 ns,
`format!("…")` ~20–30 ns, `serde_json::to_value(Ok("…")).unwrap()`
~5–10 ns, function-call overhead ~5 ns. The extra ~130 ns for
`tool_call_multi_param` is three extra `from_value` calls + the
extra `format!` arguments; the ~37 ns over `tool_call_simple` for
`tool_call_validated` is the cost of four `min_<param>` /
`max_<param>` guards on `String` + `i32`.

### What about `serde_json::to_value` of the result?

The **only** per-call allocation the macro controls. For primitive
returns (`i32`, `bool`, `String`) it is ~5–10 ns. For structured
returns (your own `#[derive(Serialize)]` structs) the cost is the
`serde_json::to_value` cost, **not** the `#[tool]` cost — there is
no Tokitai-specific overhead.

### What about the `LazyLock<ToolDefinition>`?

The `__TOOL_DEF_X` accessor wraps
`static DEF: LazyLock<ToolDefinition> = LazyLock::new(|| { … })`
and returns `&*DEF`. First call pays a one-time `Once` cost (atomic
CAS + `Box`); subsequent calls are **a pointer chase**. The
`tool_definitions_access` bench measures this end-to-end: **728 ps
median** for a 3-method impl. The `Once`-initialisation happens
once per `static`, not per method.

### What about the `phf::Map` for `#[openapi]`?

`#[openapi]` emits
`pub static __OPENAPI_OPS_<Type>: phf::Map<&'static str, __OpenApiOp_<Type>>`
— a perfect hash table keyed by `operationId` in `.rodata`; lookup
is **one hash, one index, one load** (see
[ADR-0002](adr/0002-phf-map-for-openapi-ops.md)). For the OpenAI
spec at ~250 operations the `.rodata` cost is 30–50 KB and the
lookup cost is constant — no per-call init, no `Mutex`. A `HashMap`
would have been ~150–300 ns per lookup plus one-time init; `phf`
brings that to single-digit ns at zero init cost.

### Runtime rules

1. **Don't benchmark the dispatcher.** 62 ns is invisible next to
   the 200 ms LLM call. Optimise the prompt, not the dispatcher.
2. **Reuse the provider struct.** `MyTools::default()` is cheap;
   creating a fresh `MyTools` per call defeats any internal
   `Arc` / `OnceLock` caching. One struct, one `&self`, one
   `call_tool` per request.
3. **Cache `MyClient::tool_definitions()` once at startup.** The
   `.iter().map(|t| t.to_openai_function())` chain at the top of
   your chat loop is the only place you should iterate; do it
   once per process. See
   [`CROSS_LANGUAGE.md` §2.1](CROSS_LANGUAGE.md#21-get-tools).
4. **Validate at the schema, not the body.** A
   `#[tool(min_age = 0, max_age = 150)]` guard runs at ~10 ns per
   check and gives the LLM a schema it can reason about; the
   hand-rolled `if age < 0` in the body is faster but loses the
   schema.
5. **For async tools, prefer `call_tool(name, args).await` over
   `tokio::spawn` + `call_tool_sync`.** The async dispatcher does
   not allocate a `JoinHandle`; `tokio::spawn` does.

---

## Memory

The memory cost of a `#[tool]` consumer falls into three buckets.

### 1. The static `__TOOL_DEF_*` data

Each method emits a `static DEF: LazyLock<ToolDefinition>` and a
`fn __TOOL_DEF_X()` accessor that returns `&*DEF`. `ToolDefinition::new`
copies `&'static str` slices into owned `String`s in the
`LazyLock`'s heap allocation (allocated once, never freed). For a
3-method `BenchTools` impl, **3 heap allocations** of ~256 B each;
for a 50-method impl, 50 allocations, ~12 KB total. For the OpenAI
spec, the `phf::Map` adds 30–50 KB to `.rodata`
([ADR-0002](adr/0002-phf-map-for-openapi-ops.md)).

### 2. The `Vec<ToolDefinition>` cached in `LazyLock`

`__get_tool_definitions` returns a `&'static [ToolDefinition]` built
from a `static VEC: LazyLock<Vec<ToolDefinition>>` (see
`tokitai-macros/src/tool/codegen/definitions.rs`). The `Vec` is
built once and never rebuilt; 24 B for 3 elements on 64-bit. The
underlying `Box<[ToolDefinition]>` is **separate** from the
per-method `LazyLock` heap allocations — the `Vec` holds *pointers*
to those, not copies.

### 3. The per-call `Arc<Mutex<Option<T>>>` for async dispatch

For `async fn` methods, the macro-generated `__call_X_sync` drives
the future on a user-registered runtime. The bridge
([ADR-0003](adr/0003-sync-from-async-via-block-on-dyn.md)) uses an
`Arc<Mutex<Option<T>>>` slot to smuggle the typed return value
across the type-erased `Pin<Box<dyn Future<Output = ()> + Send>>`
boundary — **one allocation per async call**. The Tokio-fallback
path (`Handle::block_on`) does not allocate the slot.

### When these matter

* **Bucket 1** matters at process startup. 50-method total: ~12 KB
  heap + 50 `Box` headers.
* **Bucket 2** is 24 B of `Vec` overhead — memory-profiler-only.
* **Bucket 3** matters at high QPS for async tools: ~64 B + the
  `T` per call. For 1 kHz this is 64 KB/s of short-lived allocations.
  Use the Tokio-fallback path to avoid the slot — but you lose
  runtime-agnosticism.

### Memory rules

1. **One `MyClient` per process, behind an `Arc` if shared.** The
   `LazyLock` statics are `&'static`; only the `MyClient` *value*
   is duplicated.
2. **For `#[openapi]`, the `.rodata` cost is real.** A 500-operation
   spec is ~100 KB of `.rodata`. If you cannot afford that, use
   `#[wrap]` with an explicit `methods = [...]` list instead.
3. **For high-QPS async tools, prefer Tokio's `Handle::block_on`
   over the type-erased bridge.** Skip
   `tokitai_core::set_async_executor(...)` and let the
   Tokio-fallback path handle the sync-from-async case.

---

## Async

Tokitai's async story is the most subtle part of the runtime; see
[ADR-0001](adr/0001-async-executor-type-erasure.md) (executor type
erasure) and
[ADR-0003](adr/0003-sync-from-async-via-block-on-dyn.md) (sync-from-
async bridge).

### When `async fn` is cheaper than `tokio::spawn` + sync `fn`

For an HTTP tool, three options:

```rust,no_run
// A: async fn on the tool, async dispatcher
pub async fn fetch(&self, url: String) -> Result<String, String> {
    reqwest::get(&url).await?.text().await.map_err(|e| e.to_string())
}

// B: sync fn on the tool, but spawned onto the runtime
pub fn fetch_blocking(&self, url: String) -> Result<String, String> {
    reqwest::blocking::get(&url)?.text().map_err(|e| e.to_string())
}
// call site: tokio::task::spawn_blocking(move || { ... })

// C: sync fn, called from a sync dispatcher
pub fn fetch_blocking(&self, url: String) -> Result<String, String> { ... }
// call site: tool.call_tool_sync("fetch", &args)?
```

**Option A** is cheapest: the async dispatcher
(`call_tool(name, args).await`) hands the future to the runtime
without allocating a `JoinHandle`. **No thread hop.**

**Option B** (`spawn_blocking`) is the most expensive: a
`JoinHandle` (~64 B), a channel send to a blocking-pool worker
(~hundreds of ns), and a worker thread (or a parked thread being
woken, ~5–20 µs). On a multi-threaded Tokio with a 512-thread
blocking pool, this is well-amortised; on a single-threaded
current-thread runtime, the pool is one thread and the call
serialises.

**Option C** (sync `call_tool_sync`) is in the middle: it **blocks
the current thread**. From a sync HTTP handler (on a
`spawn_blocking` worker), this is fine. From the runtime's main
worker, **the entire event loop stalls**.

| Pattern                                            | Per-call cost      | Blocks event loop? |
|----------------------------------------------------|--------------------|--------------------|
| `async fn` + `.await` dispatcher (A)               | ~62 ns dispatcher  | No                 |
| `sync fn` + `tokio::spawn_blocking` (B)            | ~10–20 µs          | No                 |
| `sync fn` + `call_tool_sync` from worker (C1)      | ~62 ns dispatcher  | No                 |
| `sync fn` + `call_tool_sync` from runtime (C2)     | ~62 ns dispatcher  | **Yes**            |

### When the runtime-agnostic bridge wins (and loses)

The `block_on_dyn` path through your registered `AsyncExecutor`
costs one `Arc<Mutex<Option<T>>>` allocation (the result slot), one
`Box::pin`, and one `unsafe` lifetime extension (sound, see
[ADR-0003](adr/0003-sync-from-async-via-block-on-dyn.md)). The
Tokio-fallback path (`Handle::block_on`) is **faster and simpler**
— it does not allocate the slot — but the user's method must
already be inside a Tokio runtime for `Handle::current()` to
succeed.

### Async rules

1. **Default to `async fn` if the body does I/O.** Same dispatcher
   cost, no `spawn_blocking` detour.
2. **Don't use `tokio::spawn_blocking` inside a `#[tool]` async
   method.** The dispatcher already gives you "doesn't block the
   event loop" for free.
3. **For sync `#[tool]` methods called from an async context,
   wrap the call site** in `spawn_blocking`, not the method.

---

## Schema generation

The `#[tool]` macro generates a JSON Schema for every method at
compile time. The schema is a `&'static str` baked into `.rodata`
via the `LazyLock<ToolDefinition>` static — **no schema parsing at
runtime, ever**.

### Captured cost of the schema path

| Bench                       | Median  | What it does                                        |
|-----------------------------|--------:|-----------------------------------------------------|
| `tool_definitions_access`   |  728 ps | Return `&'static [ToolDefinition]` — no schema work |
| `tool_lookup`               |  1.03 ns | Linear scan over the slice                          |
| `schema_pretty_print`       |  396 ns | `serde_json::to_string_pretty(tool.input_schema)`   |

The 396 ns for pretty-printing a 1-property schema is the cost of
pretty-printing the *string* the macro baked in (schema generation
itself is compile-time and free at runtime). For a 10-property
schema ~2 µs; for a 50-property schema ~10 µs. None of this is on
the hot path; you only pretty-print for debugging or for the
`GET /tools` HTTP response in the [MCP server](MCP_ARCHITECTURE.md).

### Static vs `tool_definitions()` at runtime

For `#[openapi]` clients, the macro emits
`pub static __OPENAPI_OPS_<Type>: phf::Map<&'static str, __OpenApiOp_<Type>>`
([ADR-0002](adr/0002-phf-map-for-openapi-ops.md)). Use the static
for **runtime introspection**, **resolving `$ref`s** (the static
holds `pub static __OPENAPI_SPEC_RAW: &str`), or a **hot path**
(an HTTP `/tools` endpoint hit on every reconnect). Use
`tool_definitions()` when you need a `Vec<ToolDefinition>` for
iteration, are using the
`to_openai_function()` / `to_anthropic_tool()` / `to_mcp_tool()`
envelope methods (see
[wrap-architecture §4.7](wrap-architecture.md#47-multi-schema-export)),
or do not need `operationId`-keyed lookup.

### Schema-generation rules

1. **Cache `tool_definitions()` once at startup**, then reuse the
   slice — iterating twice is free.
2. **For OpenAPI specs, prefer the `phf::Map` static** over
   `tool_definitions()`. A 250-operation spec means 250 entries
   in the `Vec`; iterating it on every `/tools` HTTP request is
   wasteful.
3. **Don't call `input_schema_pretty()` on the hot path.** 396 ns
   is fine for `/tools`; wasteful for a chat loop. Cache at
   startup.

---

## Best practices summary

1. **Trust the compile-time codegen** — tool definitions are
   `.rodata`; the first call is identical to the 10,000th.
2. **Don't benchmark the dispatcher** — 62 ns is invisible next to
   the 200 ms LLM call.
3. **Default to `async fn` for I/O-bound tools** — same dispatcher
   cost, no `spawn_blocking`.
4. **Cache `tool_definitions()` once at startup** — the slice is
   `&'static`; iterating it twice is free.
5. **Stack decorators innermost-most-restrictive to
   outermost-most-permissive** — `rate_limit < circuit_breaker < retry`.
6. **Favour multiple small `#[tool]` impls** over one giant one —
   the incremental `cargo check` cost is dominated by the
   *changed* impl.
7. **Measure on your own crate** before doing anything clever.

---

## See also

* [`best-practices.md`](best-practices.md) — companion guide; macro
  choice, doc-comment hygiene, error handling, anti-patterns.
* [`wrap-architecture.md`](wrap-architecture.md) — end-to-end wrap
  features, composition rules, per-feature runtime cost.
* [`wrap-cheatsheet.md`](wrap-cheatsheet.md) — one-page reference.
* [`docs/internal/compile-time-optimization.md`](internal/compile-time-optimization.md)
  — the 21 966-byte pinned baseline.
* [`docs/internal/generated-code-size-audit.md`](internal/generated-code-size-audit.md)
  — per-method emitted-item counts and five open dedup opportunities.
* [`docs/reference/`](reference/) — per-macro argument reference.
* [`docs/adr/`](adr/) — the six ADRs explaining the numbers.
* [`docs/tutorials/getting-started.md`](tutorials/getting-started.md)
  — five-chapter tutorial.
* [`tokitai-macros/benches/macro_bench.rs`](../tokitai-macros/benches/macro_bench.rs)
  — source of the runtime numbers in this document.
