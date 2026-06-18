# `#[rate_limit]`

> Pre-guard a function with a lock-free token-bucket rate limiter. The
> first `burst` calls proceed immediately; subsequent calls are
> throttled to `rps` per second. Works on both sync and async
> functions; the async path drives the throttling sleep through a
> registered `AsyncExecutor`.

## Syntax

```rust,ignore
#[rate_limit(rps = 10, burst = 20)]
fn log_event(&self, message: String) -> String { /* body */ }
```

`#[rate_limit]` is a **function-level** attribute. It accepts two
integer arguments. It is commonly stacked on `#[tool]` methods to
throttle per-endpoint call rates.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `rps` | `u32` | `1` | Sustained requests per second. Internally clamped to `>= 1` (so `rps = 0` is treated as `rps = 1`). |
| `burst` | `u32` | `1` | Maximum burst size. Internally clamped to `>= 1`. |

The interval between tokens is `1_000_000_000 / rps` nanoseconds. The
bucket starts full at `burst` tokens and refills on demand.

## Examples

### Minimal

```rust,ignore
use tokitai_macros::rate_limit;

#[rate_limit(rps = 1, burst = 1)]
fn once_per_second(&self) -> u32 { 42 }
```

### Common usage

```rust,ignore
use tokitai::tool;
use tokitai_macros::rate_limit;

#[tool]
impl Logger {
    /// Emit a log line, throttled to 10 messages per second with
    /// a 20-message burst.
    #[rate_limit(rps = 10, burst = 20)]
    pub fn log_event(&self, message: String) -> String {
        format!("logged: {}", message)
    }
}
```

### Edge case

Async function with `AsyncExecutor` registration — the throttling
sleep does **not** block a Tokio worker:

```rust,ignore
use tokitai_macros::rate_limit;

#[rate_limit(rps = 100, burst = 50)]
async fn throttled_call(&self, url: String) -> Result<String, String> {
    self.http.get(&url).send().await.map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())
}

// In main:
//   tokitai_core::set_async_executor(|| Box::pin(tokio::spawn(...)));
```

Without an executor, the throttling sleep falls back to
`std::thread::sleep`, which is OK for a sync function but will block
the calling Tokio worker on an async function.

## Generated code

For
`fn log_event(&self, message: String) -> String { body }` annotated
with `#[rate_limit(rps = 10, burst = 20)]`, the macro replaces the
body with:

```rust,ignore
{
    static __RL_TOKENS: ::std::sync::atomic::AtomicU32 =
        ::std::sync::atomic::AtomicU32::new(20u32);
    static __RL_LAST_NS: ::std::sync::atomic::AtomicU64 =
        ::std::sync::atomic::AtomicU64::new(0u64);
    use ::std::sync::atomic::Ordering;

    let __rl_interval_ns: u64 = 100_000_000u64;       // 1e9 / 10
    let __rl_burst: u32 = 20u32;
    'rate_limit_loop: loop {
        let __rl_now_ns: u64 = ::std::time::SystemTime::now()
            .duration_since(::std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0u64);
        let __rl_last_ns: u64 = __RL_LAST_NS.load(Ordering::Relaxed);
        let __rl_tokens: u32 = __RL_TOKENS.load(Ordering::Relaxed);
        let __rl_elapsed: u64 = __rl_now_ns.saturating_sub(__rl_last_ns);
        let __rl_refill: u32 = (__rl_elapsed / __rl_interval_ns) as u32;
        let __rl_new_tokens: u32 =
            if (__rl_tokens as u64) + (__rl_refill as u64) > __rl_burst as u64 {
                __rl_burst
            } else {
                __rl_tokens + __rl_refill
            };
        if __rl_new_tokens > 0u32 {
            if __RL_TOKENS
                .compare_exchange(
                    __rl_tokens,
                    __rl_new_tokens - 1u32,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                if __rl_refill > 0u32 {
                    __RL_LAST_NS.store(__rl_now_ns, Ordering::Relaxed);
                }
                break 'rate_limit_loop;
            }
        } else {
            let __rl_wait_ns: u64 =
                __rl_interval_ns - (__rl_elapsed % __rl_interval_ns);
            if false /* is_async */ {
                // T-004: drive the wait through `async_sleep`,
                // a runtime-agnostic future that yields to the
                // active executor (Tokio / async-std / smol / ...)
                // instead of blocking the runtime worker.
                ::tokitai_core::async_sleep(
                    ::std::time::Duration::from_nanos(__rl_wait_ns),
                ).await;
            } else {
                ::std::thread::sleep(
                    ::std::time::Duration::from_nanos(__rl_wait_ns),
                );
            }
        }
    }

    // original body runs here
    format!("logged: {}", message)
}
```

For **async** functions the `if false /* is_async */` branch is
replaced with `if true` and the body is wrapped in
`async move { … }.await`. The sleep itself becomes an awaited
future, so the runtime worker is never blocked.

State is held in two **function-local statics**: `__RL_TOKENS` and
`__RL_LAST_NS`. These are per-function, so multiple `#[rate_limit]`
decorators on different methods do not collide.

The CAS loop is a single 32-bit `compare_exchange` per call; the
implementation is **lock-free** under the standard "no priority
contention" assumptions.

Source:
[`tokitai-macros/src/tool/resilience/rate_limit.rs`](../../tokitai-macros/src/tool/resilience/rate_limit.rs)
(`pub fn expand`).

## Interactions

- **With `#[tool]` / `#[wrap]` / `#[openapi_op]`**: the generated
  `__call_<name>` wrapper invokes the rate-limited function, so
  throttling happens transparently to the dispatcher.
- **With `#[retry]`**: `#[rate_limit]` is innermost (applied first),
  `#[retry]` is outermost. The retry loop wraps the rate-limited
  call. See the wrap-architecture doc
  [Composition rules](../wrap-architecture.md#5-composition-rules).
- **With `#[circuit_breaker]`**: same — `#[rate_limit]` is innermost,
  `#[circuit_breaker]` is outermost. See
  [`circuit-breaker.md`](circuit-breaker.md).
- **Async executor**: as of T-004 (0.5.2) the throttling wait on
  `async fn` is driven by `tokitai_core::async_sleep(...)`, which
  yields to whatever executor is in scope (Tokio, async-std,
  smol, ...) and never blocks the calling runtime worker thread.
  Registering an `AsyncExecutor` is recommended for hot paths
  but no longer required to avoid the runtime-blocking
  `std::thread::sleep` fallback.
- **Per-function statics**: nesting two `#[rate_limit]`s on the same
  function compiles, but the inner static shadows the outer one in
  terms of token count. v2 will detect existing `__RL_*` statics and
  compose them.

## Errors

| Trigger | Message |
|---|---|
| Unknown key | `"unknown #[rate_limit] arg: <key>"` |
| `rps` not parseable as `u32` | (syn parse error) |
| `burst` not parseable as `u32` | (syn parse error) |
| `rps = 0` | silently clamped to `1` (so interval is 1 second) |
| `burst = 0` | silently clamped to `1` |

The macro produces no warnings. It also does not validate that the
function signature is reasonable; the rate-limit guard is purely
additive.

## See also

- Tutorial: [`docs/USAGE.md`](../USAGE.md) (`#[rate_limit]` section).
- Architecture: [`docs/wrap-architecture.md`](../wrap-architecture.md)
  (§4.5 — full deep-dive, including the atomic-dance diagram).
- Cheatsheet: [`docs/wrap-cheatsheet.md`](../wrap-cheatsheet.md).
- Rustdoc:
  [`tokitai-macros/src/lib.rs`](../../tokitai-macros/src/lib.rs)
  (`pub fn rate_limit`).
- **tracking-issue:** [#34](https://github.com/silverenternal/tokitai/issues/34) (attribute not yet exported in 0.5.x).
- Example: [`examples/runtime_agnostic.rs`](../../examples/runtime_agnostic.rs)
  shows how to register an `AsyncExecutor`.
