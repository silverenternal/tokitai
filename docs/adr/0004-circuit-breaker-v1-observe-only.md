# ADR-0004: `#[circuit_breaker]` v1 is observe-only, not fail-fast

- **Status:** Accepted
- **Date:** 2026-06-02
- **Authors:** Tokitai maintainers

## Context

`#[circuit_breaker]` is one of three resilience decorators
(`#[retry]`, `#[rate_limit]`, `#[circuit_breaker]`) added in v0.4.0
to wrap function bodies with cross-cutting concerns. The decorator
implements a 3-state machine (closed / open / half-open) on top of
three static atomics per decorated function.

A "true" circuit breaker does two things:

1. **Observes** success and failure counts, transitioning between
   closed / open / half-open.
2. **Short-circuits** calls when the circuit is open, returning
   an error without invoking the body.

The classic fail-fast contract is: when the circuit is open,
return `Err(CircuitOpen::new())` immediately. For this to be
generic over the user's error type, `E` must implement
`From<String>` or a similar trait that lets the macro synthesise
a "circuit open" error.

The trap: requiring `E: From<String>` (or a `CircuitOpen` trait)
on every method the user decorates is a **non-trivial constraint
on the macro signature**. Users with custom error types
(`MyServiceError::RateLimited`, `anyhow::Error`,
`thiserror::Error` enums) would not be able to apply the
decorator without first adding a `From<String>` impl, which is
precisely the kind of friction the rest of Tokitai (which is
`From`-free on parameter types) avoids.

## Decision

v1 of `#[circuit_breaker]` is **observe-only**: the macro records
the success/failure state in three static atomics, transitions
between closed / open / half-open, and updates the public counters,
but **does not short-circuit** subsequent calls when the circuit is
open. The body still runs; the user can read the state via the
public `static` to write their own short-circuit logic.

The relevant generated code
([`tokitai-macros/src/tool/resilience/circuit_breaker.rs`](../../tokitai-macros/src/tool/resilience/circuit_breaker.rs)):

```rust
if __cb_state == 1u8 {
    if elapsed >= __cb_reset_ns {
        __CB_STATE.store(2u8, Ordering::Relaxed); // -> HalfOpen
    }
    // v1 still runs the body; v2 will add a fail-fast early-return.
}
let __cb_inner = async move #original_block;
match __cb_inner.await {
    Ok(v)  => { /* reset state */ Ok(v) }
    Err(e) => { /* record failure, possibly open */ Err(e) }
}
```

The state atomics are emitted as `static` items at the outermost
block scope, so the user can address them by name.

## Consequences

**Easier:**

- The macro is drop-in: any `fn(&self, ...) -> Result<T, E>` works,
  regardless of `E`. No `E: From<String>` bound, no `CircuitOpen`
  trait, no "which error types are supported" documentation.
- Users who want fail-fast can read the state with a one-liner at
  the top of their function and `return Err(...)` themselves. This
  also gives them full control over the synthesised error: a
  structured error type with metadata, not just a string.
- v1 is safe to stack with any other decorator; no implicit
  short-circuit that would surprise the user.

**Harder:**

- The decorator does not, by itself, protect a downstream
  service from being hammered. Users have to write the
  short-circuit themselves. We accepted this for v1 because
  (a) the three-statics shape makes the short-circuit trivial,
  and (b) we are committed to shipping v2 with a real fail-fast.
- The "this looks like a circuit breaker but it does not
  short-circuit" surprise has a non-trivial chance of catching
  a user who copies an example and assumes the macro is doing
  the protecting. The `#[doc = "v1 limitation: ..."]` note on
  the macro is the only defense.
- The state atomics are emitted as `pub(crate)` re-exports, so
  external users cannot read them directly. They can read them
  via the `static` items the macro emits in the same scope, but
  the `__CB_` prefix makes them easy to miss.

## Alternatives considered

- **Make fail-fast the default** — rejected. Fail-fast forces
  `E: From<String>` (or the new `CircuitOpen` trait) on every
  decorated method. Users with custom error types would have
  to wrap their error in a `String`-convertible type or
  implement the trait. Both are friction, and neither is
  discoverable from the macro signature alone.
- **Require a specific error type (`thiserror::Error` or
  `anyhow::Result`)** — rejected. Picks winners in the
  error-handling ecosystem. Tokitai is intentionally agnostic
  on this; the resilience decorators should be too.
- **Two-attribute approach: `#[circuit_breaker_observe]` and
  `#[circuit_breaker_failfast]`** — rejected. Two attributes
  for one decorator is a code smell. v1 ships one, v2 will
  introduce `#[circuit_breaker(fail_fast = true)]` as an
  opt-in argument.
