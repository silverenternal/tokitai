# `#[retry]`

> Wraps the body of a `Result`-returning function in a retry loop with
> configurable backoff and jitter. Works on both sync and async
> functions; async sleep is driven through a registered
> `AsyncExecutor` so it does not block a runtime worker thread.

## Syntax

```rust,ignore
#[retry(max = 3, backoff = "exponential", jitter = true, on = "any")]
async fn fetch(&self, url: String) -> Result<String, String> { /* body */ }
```

`#[retry]` is a **function-level** attribute (not `impl`-level, not
method-level in the `#[tool]` sense — it works on any function whose
return type is `Result<T, E>`). It is commonly stacked on `#[tool]`
methods.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `max` | `u32` | `3` | Maximum number of attempts (including the first). |
| `backoff` | `&str` | `"exponential"` | One of `"constant"`, `"linear"`, `"exponential"`. |
| `jitter` | `bool` | `true` | Add a small random offset (0–50 ms) derived from `SystemTime::now().subsec_nanos()`. |
| `on` | `&str` | `"any"` | Forward-compat only. v1 always retries on any `Err`. |

### Backoff formulas

| `backoff` | Sleep before attempt N (N = 1, 2, 3, …) |
|---|---|
| `"constant"` | `100 ms` |
| `"linear"` | `100 * N ms` |
| `"exponential"` | `100 * 2^(N-1) ms` (capped at N = 21) |

The `N`-th attempt is preceded by the N-th sleep, so the first call
goes through immediately.

## Examples

### Minimal

```rust,ignore
use tokitai_macros::retry;

#[retry(max = 3)]
async fn ping(&self) -> Result<bool, String> {
    Ok(true)
}
```

### Common usage

```rust,ignore
use tokitai::tool;
use tokitai_macros::retry;

#[tool]
impl WeatherClient {
    /// Fetch the weather for `city`. Up to 5 attempts, exponential
    /// backoff with jitter.
    #[retry(max = 5, backoff = "exponential", jitter = true)]
    pub async fn get_weather(&self, city: String) -> Result<Weather, Error> {
        self.http
            .get(format!("https://api.weather.example/{city}"))
            .send().await?
            .json().await
    }
}
```

### Edge case

`#[retry]` on a **sync** function: the same loop, but the sleep is
`std::thread::sleep` unconditionally (no async executor needed).

```rust,ignore
use tokitai_macros::retry;

#[retry(max = 3, backoff = "constant", jitter = false)]
fn sync_read(&self, path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}
```

If the body is non-`Result`, the macro still expands (the loop's match
just always goes through the `Ok` arm).

## Generated code

For an async `fn fetch(&self, url: String) -> Result<String, String> { body }`,
`#[retry(max = 3, backoff = "exponential", jitter = true)]` replaces
the body with:

```rust,ignore
{
    let mut attempt: u32 = 0u32;
    let __retry_result = loop {
        attempt = attempt + 1u32;
        let __r = async move {
            // …original body…
        };
        match __r.await {
            Ok(__v) => break Ok(__v),
            Err(__e) if attempt < 3u32 => {
                let __backoff_ms: u64 =
                    100u64 * (1u64 << (::std::cmp::min(attempt, 20u32).saturating_sub(1)));
                let __jitter_offset: u64 = {
                    let __nanos = ::std::time::SystemTime::now()
                        .duration_since(::std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as u64)
                        .unwrap_or(0u64);
                    __nanos % 50u64
                };
                let __total_ms: u64 = __backoff_ms.saturating_add(__jitter_offset);
                {
                    let __retry_sleep = ::std::time::Duration::from_millis(__total_ms);
                    if ::tokitai_core::current_async_executor().is_some() {
                        let __retry_res: ::std::result::Result<(), ::tokitai_core::ToolError> =
                            ::tokitai_core::block_on_async(async move {
                                ::std::thread::sleep(__retry_sleep);
                            });
                        let _ = __retry_res;
                    } else {
                        ::std::thread::sleep(__retry_sleep);
                    }
                }
            }
            Err(__e) => break Err(__e),
        }
    };
    __retry_result
}
```

For **sync** functions the async `await` and the `AsyncExecutor` block
are dropped; the sleep is `std::thread::sleep` directly.

No statics are allocated; no `LazyLock`; no runtime state. The
function's signature is preserved verbatim.

Source:
[`tokitai-macros/src/tool/resilience/retry.rs`](../../tokitai-macros/src/tool/resilience/retry.rs)
(`pub fn expand`).

## Interactions

- **With `#[tool]` / `#[wrap]` / `#[openapi_op]`**: the generated
  `__call_<name>` wrapper invokes the function, so retrying happens
  transparently to the dispatcher.
- **With `#[rate_limit]`**: `#[rate_limit]` is the inner attribute
  (innermost) and `#[retry]` is the outer one. The retry loop wraps
  the rate-limited call. See the wrap-architecture doc
  [Composition rules](../wrap-architecture.md#5-composition-rules).
- **With `#[circuit_breaker]`**: same — `#[circuit_breaker]` is
  innermost, `#[retry]` is outermost. See
  [`circuit-breaker.md`](circuit-breaker.md).
- **Async executor**: register one with
  `tokitai_core::set_async_executor(...)` at program startup. Without
  one, async `#[retry]` falls back to `std::thread::sleep`, which
  blocks the calling runtime worker thread.
- **Nested `#[retry]`**: in v1, the inner layer wins. The outer
  attribute is applied first, sees a `Result`-returning body, and
  wraps it — but the inner layer has already been expanded, so the
  effective `max` is the inner one. v2 will append layers.

## Errors

| Trigger | Message |
|---|---|
| Unknown key | `"unknown #[retry] arg: <key>"` |
| `max` not parseable as `u32` | (syn parse error, surfaces as a standard compile error) |
| `backoff` not one of `"constant"`, `"linear"`, `"exponential"` | (silently falls back to `100 ms` constant) |
| `jitter` not a literal `bool` | (syn parse error) |

The macro produces no warnings. It also does not validate that the
function actually returns a `Result`; if it doesn't, the generated
loop's match arm always takes the `Ok` branch and the loop runs
exactly once.

## See also

- Tutorial: [`docs/USAGE.md`](../USAGE.md) (`#[retry]` section).
- Architecture: [`docs/wrap-architecture.md`](../wrap-architecture.md)
  (§4.4 — full deep-dive, including the backoff math).
- Cheatsheet: [`docs/wrap-cheatsheet.md`](../wrap-cheatsheet.md).
- Rustdoc:
  [`tokitai-macros/src/lib.rs`](../../tokitai-macros/src/lib.rs)
  (`pub fn retry`).
- Example: [`examples/resilient_tool.rs`](../../examples/deprecated/resilient_tool.rs) (placeholder; see [`deprecated/`](../../examples/deprecated/)).
- Example: [`examples/runtime_agnostic.rs`](../../examples/runtime_agnostic.rs)
  shows registering an `AsyncExecutor` so `#[retry]` does not block
  a Tokio worker.
