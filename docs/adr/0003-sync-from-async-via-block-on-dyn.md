# ADR-0003: Sync-from-async bridge uses `block_on_dyn`, not `block_on_async`

- **Status:** Accepted
- **Date:** 2026-06-02
- **Authors:** Tokitai maintainers

## Context

`#[tool]` generates a sync `__call_<name>_sync` wrapper for every
`async fn` in an `impl` block, so the generated `call_tool_sync`
dispatcher can call the method from a non-async context. The wrapper
has to drive the future to completion on the user's runtime.

The generated future **borrows `&self`** with a lifetime that is
**not `'static`**: the wrapper signature is
`fn __call_foo<'a>(&'a self, args: &'a serde_json::Value) -> ...`,
and the future produced by `async move { self.method(...).await }`
captures the `&'a self` borrow. This rules out `block_on_async`,
which requires `F: 'static`.

The `tokitai_core` crate ships two entry points:

- `block_on_async<F: Future + 'static>(F) -> F::Output` — typed,
  used by user code that holds a `'static` future.
- `block_on_dyn` on the `AsyncExecutor` trait — type-erased, only
  requires `Send`, used by macro-generated code.

This ADR documents the **macro-level** choice: which entry point the
sync-from-async bridge uses, and why we have to lie about lifetimes
to get there.

## Decision

The sync-from-async bridge in the macro output drives the future
through the registered executor's `block_on_dyn`. The bridge lifts
the non-`'static` future to `'static` with a sound `unsafe`
transmute, because the executor drives the future to completion
synchronously before `block_on_dyn` returns.

The relevant generated code
([`tokitai-macros/src/tool/codegen/wrappers.rs`](../../tokitai-macros/src/tool/codegen/wrappers.rs)):

```rust
let __pinned: Pin<Box<dyn Future<Output = ()> + Send + 'a>>
    = Box::pin(__fut); // captures &self

// SAFETY: the executor drives the future to completion
// synchronously; the future is dropped at the end of this scope.
let __pinned: Pin<Box<dyn Future<Output = ()> + Send + 'static>>
    = unsafe { core::mem::transmute(__pinned) };

let _ = _exec.block_on_dyn(__pinned);
```

The result is read out of an
`Arc<Mutex<Option<<method return type>>>` that the inner `async
move` block wrote into.

## Consequences

**Easier:**

- The generated sync wrapper works against any executor the user
  installs — Tokio, `async-std`, `smol`, a custom one — without the
  macro having to know which. The macro only needs `block_on_dyn`,
  which the `AsyncExecutor` trait guarantees.
- The `Tokio` runtime fallback (used when no executor is registered)
  is implemented separately. `Handle::block_on` is generic over the
  future's lifetime, so it can consume the non-`'static` future
  directly without the `unsafe` lift.

**Harder:**

- The bridge allocates a result slot (`Arc<Mutex<Option<T>>>`) per
  call. This is one heap allocation plus a `Mutex` lock per
  sync-from-async invocation. For tool calls (which already parse
  JSON and dispatch through a `match`) this is in the noise.
- The `unsafe` lifetime extension is sound **only** because the
  executor contract requires synchronous drive. A future executor
  that returns before the future completes would introduce a
  use-after-free; the trait docs are the only thing standing
  between us and that footgun.
- The `Result<Value, ToolError>` shape of the outer
  `result_handling` block used to double-wrap the result
  (showing up as `{"Ok": <value>}` to callers). The bridge now
  stores the **raw** return value in the slot and lets the
  outer block do the single wrap. This split was a real bug
  we had to track down.

## Alternatives considered

- **Require all async tools to be called from inside a Tokio
  runtime** — rejected. The whole point of the `AsyncExecutor` trait
  is to be runtime-agnostic. Forcing Tokio would mean `#[tool]` is
  useless to `async-std` and `smol` users.
- **Generate a separate "async-only" trait** — rejected. The whole
  reason to have `#[tool]` is that the same `ToolProvider` /
  `ToolCaller` impl works in both sync and async contexts. Splitting
  the trait doubles the API surface and the documentation burden.
- **Make all generated methods `async`** — rejected. Half the
  consumer code in `examples/` calls `call_tool` from a sync context
  (HTTP handlers, CLI entry points, blocking test harnesses). Making
  every method `async` would force every consumer to be `async`.
