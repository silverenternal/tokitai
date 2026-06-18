//! Resilience decorator macros: `#[retry]`, `#[rate_limit]`,
//! `#[circuit_breaker]`.
//!
//! These are per-method proc-macro attributes that wrap the body of a
//! function (sync or async) with retry, rate-limiting, or
//! circuit-breaker logic. They are designed to be used either
//! standalone on any function, or inside a `#[tool]` impl block
//! (where the wrapping is baked into the function before the
//! `#[tool]` macro sees it).
//!
//! All three are deliberately written without new dependencies — the
//! generated code relies only on `std::sync::atomic`, `std::time`,
//! and (for the async case) the runtime-agnostic
//! `tokitai_core::async_sleep(...)` future, which yields control to
//! the active executor for the full duration of the wait. The
//! `async_sleep` helper itself does not pull in any runtime crate:
//! it spawns a single `std::thread` per wait that wakes the future's
//! `Waker` when the deadline elapses. This works for any executor
//! the user has registered — Tokio, async-std, smol, or a custom
//! one — and never blocks a runtime worker thread.
//!
//! ## Composition
//!
//! Multiple decorators may be stacked on the same method. The Rust
//! attribute-processor applies them in source order, outermost-last:
//!
//! ```ignore
//! #[retry(max = 3)]
//! #[rate_limit(rps = 10)]
//! async fn foo() -> Result<_, _> { ... }
//! ```
//!
//! `#[rate_limit]` is applied first (innermost), then `#[retry]`
//! wraps the rate-limited call in a retry loop.
//!
//! v1 of these macros enforces single-layer wrapping only. Nested
//! composition (e.g. two `#[retry]` layers) is a stretch goal — see
//! the per-module docs for the v2 design sketch.

pub mod circuit_breaker;
pub mod rate_limit;
pub mod retry;
