# ADR-0001: `AsyncExecutor` uses type erasure, not generics

- **Status:** Accepted
- **Date:** 2026-06-02
- **Authors:** Tokitai maintainers

## Context

Tokitai exposes a **runtime-agnostic** async executor bridge. The
`#[tool]` macro generates a sync wrapper for every `async fn` in an
`impl` block; that wrapper has to drive a future to completion
**without** hard-coding Tokio. The user picks the runtime
(`async-std`, `smol`, `embassy`, a custom executor) at program start,
and the generated wrapper must work for any of them.

The natural Rust shape for "executor over a future" is a trait
parameterised by the future type:

```rust
trait AsyncExecutor {
    fn block_on<F: Future>(&self, future: F) -> F::Output;
}
```

But this trait is **not object-safe**: it has a generic method, so
`dyn AsyncExecutor` is forbidden by the language. The process-wide
executor slot in `tokitai_core` therefore has to hold a **concrete**
executor type, and the consumer code is forced to thread an
`E: AsyncExecutor` type parameter everywhere it wants to call
`block_on`.

We wanted `OnceLock<Box<dyn AsyncExecutor>>` so the user can install
their executor once with `set_async_executor(Box::new(my_exec))` and
never mention the type again.

## Decision

The `AsyncExecutor` trait is object-safe. It uses
`Pin<Box<dyn Future<Output = ()> + Send>>` for the future parameter
and `Box<dyn Any + Send>` for the return value:

```rust
pub trait AsyncExecutor: Send + Sync {
    fn block_on_dyn(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send>>,
    ) -> Box<dyn Any + Send>;
}
```

To give callers back the natural typed API, we provide a companion
`AsyncExecutorExt` extension trait whose blanket impl re-introduces
`block_on<F: Future>(&self, F) -> F::Output`. The extension uses an
`Arc<Mutex<Option<F::Output>>>` to smuggle the result back across
the type-erasure boundary.

The user-registered executor is stored in a process-wide
`OnceLock<Box<dyn AsyncExecutor>>`. First registration wins.

## Consequences

**Easier:**

- The user calls `set_async_executor(Box::new(my_exec))` once at
  startup. Everything else is `dyn`-dispatched. No type parameters
  infect `ToolProvider`, `ToolCaller`, or the generated dispatchers.
- `#[tool]`'s sync wrapper can call
  `current_async_executor().expect("...").block_on_dyn(...)` without
  naming a concrete executor type.
- The trait is mockable in tests: a test-only `NullExecutor` and
  `BlockingExecutor` can be slotted in without changing call sites.

**Harder:**

- Every typed call to `block_on` allocates an `Arc<Mutex<Option<T>>>`
  for the result slot. The cost is one heap allocation per call; in
  practice this is dwarfed by the cost of driving a future.
- The `unsafe` lifetime extension (see [ADR-0003](0003-sync-from-async-via-block-on-dyn.md))
  is required to lift the non-`'static` borrow to `'static` so the
  type-erased future type-checks.
- The `AsyncExecutorExt` wrapper is a subtle piece of code. A new
  contributor reading `block_on` for the first time will see the slot
  pattern and may mistake it for a bug.

## Alternatives considered

- **Generic `AsyncExecutor`** — rejected. Every consumer type
  (`ToolCaller`, `ToolProvider`, the generated `call_tool`) would
  have to thread the executor type through, which in turn forces the
  `#[tool]` macro to know the executor type at codegen time. The
  user-facing API would be `MyClient<Exec>` instead of `MyClient`.
- **`async-trait` crate** — rejected. The crate expands to a
  near-`async fn` on stable, but adds a dependency, hides the vtable
  behind a macro, and (in 0.1) emits a `Box::pin`-per-call for the
  argument. We can get the same behaviour with ~10 lines of hand-rolled
  trait.
- **Hand-rolled vtable without `Any`** — rejected. The output type
  varies per call site; the only way to erase it without
  `Box<dyn Any>` is to add a generic method back, which is what we
  were trying to avoid.
