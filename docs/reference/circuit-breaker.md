# `#[circuit_breaker]`

> Pre/post-guard a `Result`-returning function with a 3-state
> (closed / open / half-open) circuit breaker. After
> `failure_threshold` consecutive `Err`s the breaker opens; after
> `reset_timeout` it transitions to half-open and probes with the
> next call.

## Syntax

```rust,ignore
#[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]
async fn call_external(&self, endpoint: String) -> Result<String, String> { /* body */ }
```

`#[circuit_breaker]` is a **function-level** attribute. It accepts
two arguments: an integer threshold and a human-friendly duration
string. It is commonly stacked on `#[tool]` methods to stop
hammering a known-broken dependency.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `failure_threshold` | `u32` | `5` | Consecutive `Err`s before the breaker opens. |
| `reset_timeout` | `&str` | `"30s"` | How long the breaker stays open before allowing a half-open probe. Accepts `"ms"`, `"s"`, `"m"`, `"h"` suffixes, or a bare integer (interpreted as seconds). |

### `reset_timeout` suffixes

| Suffix | Meaning |
|---|---|
| `ms` | milliseconds |
| `s` | seconds (default if bare) |
| `m` | minutes |
| `h` | hours |
| _(none)_ | seconds (e.g. `reset_timeout = "30"` → 30 s) |

The macro parses the string into a `u64` of nanoseconds. A
malformed string silently becomes `0`, which means the breaker
transitions to half-open on the very next call.

## Examples

### Minimal

```rust,ignore
use tokitai_macros::circuit_breaker;

#[circuit_breaker(failure_threshold = 3, reset_timeout = "10s")]
async fn fetch(&self) -> Result<String, String> { Ok("ok".into()) }
```

### Common usage

```rust,ignore
use tokitai::tool;
use tokitai_macros::circuit_breaker;

#[tool]
impl ExternalClient {
    /// Call an external service behind a circuit breaker that
    /// opens after 5 consecutive failures and re-tries probing
    /// after 30 seconds.
    #[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]
    pub async fn call_external(&self, endpoint: String) -> Result<String, String> {
        if endpoint.is_empty() {
            return Err("missing endpoint".to_string());
        }
        Ok(format!("called {}", endpoint))
    }
}
```

### Edge case

Sub-second timeouts and minutes / hours:

```rust,ignore
use tokitai_macros::circuit_breaker;

// Half-second reset — useful for a flaky local service.
#[circuit_breaker(failure_threshold = 3, reset_timeout = "500ms")]
async fn local_call(&self) -> Result<u32, String> { Ok(1) }

// One-hour reset — for an upstream vendor that updates its SLA
// daily.
#[circuit_breaker(failure_threshold = 10, reset_timeout = "1h")]
async fn vendor_call(&self) -> Result<String, String> { Ok("x".into()) }
```

The v1 implementation **does not** fail-fast when the circuit is
open; the body still runs, so the call observes the current state.
See the [Limitations](#limitations--v1-fail-fast) section below.

## Generated code

For
`async fn call_external(&self, endpoint: String) -> Result<String, String> { body }`
annotated with
`#[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]`,
the macro replaces the body with:

```rust,ignore
{
    // 0 = Closed, 1 = Open, 2 = HalfOpen
    static __CB_STATE: ::std::sync::atomic::AtomicU8 =
        ::std::sync::atomic::AtomicU8::new(0u8);
    static __CB_FAILURES: ::std::sync::atomic::AtomicU32 =
        ::std::sync::atomic::AtomicU32::new(0u32);
    static __CB_OPEN_AT_NS: ::std::sync::atomic::AtomicU64 =
        ::std::sync::atomic::AtomicU64::new(0u64);
    use ::std::sync::atomic::Ordering;

    let __cb_threshold: u32 = 5u32;
    let __cb_reset_ns: u64 = 30_000_000_000u64;     // 30 s
    let __cb_now_ns: u64 = ::std::time::SystemTime::now()
        .duration_since(::std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0u64);
    let __cb_state: u8 = __CB_STATE.load(Ordering::Relaxed);

    if __cb_state == 1u8 {
        let __cb_open_at: u64 = __CB_OPEN_AT_NS.load(Ordering::Relaxed);
        if __cb_now_ns.saturating_sub(__cb_open_at) >= __cb_reset_ns {
            // Transition to HalfOpen; the call below is the probe.
            __CB_STATE.store(2u8, Ordering::Relaxed);
        }
        // else: circuit is open; v1 still runs the body so the
        // call observes the current state. v2 will add a
        // fail-fast early-return here.
    }

    let __cb_inner = async move {
        // …original body…
    };
    match __cb_inner.await {
        Ok(__cb_v) => {
            __CB_STATE.store(0u8, Ordering::Relaxed);
            __CB_FAILURES.store(0u32, Ordering::Relaxed);
            Ok::<_, _>(__cb_v)
        }
        Err(__cb_e) => {
            let __cb_f = __CB_FAILURES
                .fetch_add(1u32, Ordering::Relaxed)
                + 1u32;
            if __cb_f >= __cb_threshold {
                __CB_STATE.store(1u8, Ordering::Relaxed);
                __CB_OPEN_AT_NS.store(__cb_now_ns, Ordering::Relaxed);
            }
            Err::<_, _>(__cb_e)
        }
    }
}
```

For **sync** functions the `async move { … }.await` block is
replaced with the original block directly.

State is held in three **function-local statics**: `__CB_STATE`
(`AtomicU8`), `__CB_FAILURES` (`AtomicU32`), and `__CB_OPEN_AT_NS`
(`AtomicU64`).

### State machine

| State (`__CB_STATE`) | Meaning | On entry, before call | After call |
|---|---|---|---|
| `0` Closed | calls pass through | (no-op) | `Ok` → reset failures, stay Closed; `Err` → ++failures, transition Open if `>= threshold` |
| `1` Open | calls should be throttled | if `now - open_at >= reset_ns`, transition to HalfOpen | (probe runs; outcome follows the Closed rules) |
| `2` HalfOpen | the probe call | (no-op) | `Ok` → Closed + reset; `Err` → Open + reset `open_at` |

Source:
[`tokitai-macros/src/tool/resilience/circuit_breaker.rs`](../../tokitai-macros/src/tool/resilience/circuit_breaker.rs)
(`pub fn expand` and `fn parse_duration_ns`).

## Limitations — v1 fail-fast

v1 does **not** synthesise an error when the breaker is open; the
body still runs, so the caller observes the current state. This
keeps the error type generic (no `E: From<String>` bound on the
user's error). v2 will introduce a `CircuitOpen` trait the user's
error type implements, and the macro will call
`<E as CircuitOpen>::open()` to synthesise the fast-fail error.

In v1, the workaround is to read `__CB_STATE` at the start of the
body and return early:

```rust,ignore
use tokitai_macros::circuit_breaker;
use std::sync::atomic::Ordering;

#[circuit_breaker(failure_threshold = 5, reset_timeout = "30s")]
async fn fast_fail(&self) -> Result<String, String> {
    // `__CB_STATE` is a function-scope `AtomicU8` re-exported by the
    // macro into the function body; 1 means Open. In v2 this will
    // move behind a `CircuitOpen` trait.
    if __CB_STATE.load(Ordering::Relaxed) == 1u8 {
        return Err("circuit open".to_string());
    }
    // …real body…
    Ok("ok".into())
}
```

## Interactions

- **With `#[tool]` / `#[wrap]` / `#[openapi_op]`**: the generated
  `__call_<name>` wrapper invokes the breaker-guarded function, so
  the breaker is transparent to the dispatcher.
- **With `#[retry]`**: `#[circuit_breaker]` is innermost (applied
  first), `#[retry]` is outermost. The retry loop wraps the
  breaker-guarded call. See the wrap-architecture doc
  [Composition rules](../wrap-architecture.md#5-composition-rules).
- **With `#[rate_limit]`**: same — `#[rate_limit]` is innermost,
  `#[circuit_breaker]` is outermost. See
  [`rate-limit.md`](rate-limit.md).
- **Per-function statics**: nesting two `#[circuit_breaker]`s on the
  same function compiles, but the inner static shadows the outer
  one in terms of state. v2 will detect existing `__CB_*` statics
  and compose them.

## Errors

| Trigger | Message |
|---|---|
| Unknown key | `"unknown #[circuit_breaker] arg: <key>"` |
| `failure_threshold` not parseable as `u32` | (syn parse error) |
| `reset_timeout` not parseable as a duration | (silently becomes `0 ns`) |

The macro produces no warnings. It also does not validate that the
function actually returns a `Result`; if it doesn't, the generated
match arm always takes the `Ok` branch and the breaker never opens.

## See also

- Tutorial: [`docs/USAGE.md`](../USAGE.md) (`#[circuit_breaker]`
  section).
- Architecture: [`docs/wrap-architecture.md`](../wrap-architecture.md)
  (§4.6 — full deep-dive, including the state-machine diagram).
- Cheatsheet: [`docs/wrap-cheatsheet.md`](../wrap-cheatsheet.md).
- Rustdoc:
  [`tokitai-macros/src/lib.rs`](../../tokitai-macros/src/lib.rs)
  (`pub fn circuit_breaker`).
- Example: [`examples/resilient_tool.rs`](../../examples/deprecated/resilient_tool.rs) (placeholder; see [`deprecated/`](../../examples/deprecated/)).
- Example: [`examples/runtime_agnostic.rs`](../../examples/runtime_agnostic.rs)
  (registers an `AsyncExecutor` for the resilience decorators).
