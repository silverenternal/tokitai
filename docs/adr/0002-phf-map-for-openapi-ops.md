# ADR-0002: OpenAPI operations use `phf::Map`, not `HashMap`

- **Status:** Accepted
- **Date:** 2026-06-02
- **Authors:** Tokitai maintainers

## Context

The `#[openapi(spec = "...")]` proc-macro parses an OpenAPI 3 spec at
compile time, walks every operation, and emits a lookup table from
`operationId` to the per-operation metadata. The table is read at
runtime by introspection code (`OPENAPI_OPS`, the raw `__OPENAPI_OPS_<T>`
statics) and by the generated dispatchers.

The question is what data structure to use for that table. Three
candidates were on the table:

1. A `HashMap<&'static str, OpenApiOp>` initialised lazily at first use.
2. A linear scan over a `&'static [(&'static str, OpenApiOp)]`.
3. A `phf::Map<&'static str, OpenApiOp>` constructed at compile time.

Tokitai's design principle is **all schema parsing happens at
proc-macro compile time** (see `docs/wrap-architecture.md` §3,
"Key invariants"). A `HashMap` violates that principle by deferring
the work to runtime. A linear scan is O(n) and becomes the bottleneck
once a single API has more than ~50 operations (the OpenAI spec has
hundreds).

## Decision

The generated lookup table is a `phf::Map<&'static str, OpenApiOp>`
constructed in the macro output via `phf::phf_map!`:

```rust
pub static __OPENAPI_OPS_OpenAIClient: ::phf::Map<
    &'static str,
    __OpenApiOp_OpenAIClient,
> = ::phf::phf_map! {
    "createChatCompletion" => __OpenApiOp_OpenAIClient { ... },
    "listModels" => __OpenApiOp_OpenAIClient { ... },
    // ... every operation
};
```

`phf` (perfect hash function) builds a minimal perfect hash at
**compile time** of the consumer crate. Lookup is a single hash,
single index, single load — no probing, no chaining, no allocation.

The implementation lives in
[`tokitai-macros/src/tool/wrap_openapi/spec_static.rs`](../../tokitai-macros/src/tool/wrap_openapi/spec_static.rs).

## Consequences

**Easier:**

- Lookup is O(1) with **zero runtime allocation** and **zero
  initialisation cost** — the map is in the binary's `.rodata`.
- First-call latency is deterministic. There is no "first call
  initialises the map" cliff that would show up in latency
  benchmarks.
- The map is part of the `&'static` data; it is `Send + Sync` and
  safe to share across threads without a `Mutex`.
- The `phf` crate is tiny (~30 KB of compile-time cost) and has no
  transitive dependencies.

**Harder:**

- The proc-macro output is larger because the entire map is inlined
  into the binary. For an OpenAI-sized spec (~250 operations) this is
  on the order of 30-50 KB of `.rodata`. We accepted this in exchange
  for the lookup-latency win.
- `phf` is a build-time dependency of the consumer. Users who cannot
  pull in a new crate (e.g. in a no-network build) cannot use
  `#[openapi]`. The `#[tool]` macro does not depend on `phf`.
- The static name embeds the impl block's type name, so a type named
  `OpenAIClient` produces a `__OPENAPI_OPS_OpenAIClient` static. This
  violates `SCREAMING_SNAKE_CASE`, so the macro emits
  `#[allow(non_upper_case_globals)]` on the static.

## Alternatives considered

- **`LazyLock<HashMap>` (or `OnceLock<HashMap>`)** — rejected.
  Initialisation happens on the first call. That call pays the
  allocation, hashing, and insertion cost — visible in latency
  benchmarks and on hot paths like MCP server startup. The
  initialisation is also non-deterministic in a multi-threaded
  program: whichever thread wins the `OnceLock::get_or_init` race
  pays the cost. A `phf::Map` makes the cost zero.
- **Linear scan over `&'static [(&str, Op)]`** — rejected. O(n) per
  lookup. With the OpenAI spec at ~250 operations, every introspection
  call walks 250 entries. Acceptable for `print once at startup` but
  not for a per-tool dispatcher path.
- **`BTreeMap` (immutable, const-friendly in nightly)** — rejected.
  Const `BTreeMap` is unstable; `phf` works on stable. Also slower
  per lookup than `phf`.
