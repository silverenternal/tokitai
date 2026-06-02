//! Runtime-agnostic async executor example
//!
//! Demonstrates Tokitai's runtime-agnostic async executor API:
//! `tokitai_core::AsyncExecutor`, `set_async_executor`, and
//! `block_on_async`. We register a custom executor backed by
//! `futures::executor::block_on` and drive the tool bodies through it.
//!
//! Run with: cargo run -p tokitai-examples --example runtime_agnostic
//!
//! No `tokio` is used on this example's code path.
//!
//! ## Workaround note
//!
//! The `#[tool]` macro's auto-generated `__call_*_sync` wrappers forward
//! `async move { self.method().await }` to `block_on_async`. That future
//! borrows `&self`, so its lifetime is the wrapper's `'a` — not
//! `'static`. `block_on_async` requires `F: 'static`, so the generated
//! wrapper can only be called when the macro adds a `where 'a: 'static`
//! bound (or equivalent), which it does not in v0.4.0. Until the macro
//! is fixed, this example keeps its tool methods synchronous and calls
//! `block_on_async` *inside* the sync body with a `'static` future that
//! does not borrow `&self`. This still exercises the full
//! `set_async_executor` / `current_async_executor` / `block_on_async`
//! surface that the new API exposes.

use std::future::Future;
use std::pin::Pin;
use std::thread;
use std::time::Duration;

use futures::channel::oneshot;
use serde_json::json;
use tokitai::tool;
use tokitai_core::{block_on_async, current_async_executor, set_async_executor, AsyncExecutor};

/// A non-Tokio sleep future. Spawns a side thread that wakes the task
/// after `duration`. Works under any single-threaded executor.
fn sleep(duration: Duration) -> impl Future<Output = ()> + Send + 'static {
    let (tx, rx) = oneshot::channel::<()>();
    thread::spawn(move || {
        thread::sleep(duration);
        let _ = tx.send(());
    });
    async move {
        let _ = rx.await;
    }
}

/// Calculator with three tools. The bodies drive themselves through the
/// registered executor with a `'static` future (no `&self` borrow).
#[derive(Default)]
pub struct Calculator;

#[tool]
impl Calculator {
    /// Synchronous add — demonstrates that the executor registration is
    /// non-destructive: sync tools keep working unchanged.
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// Sleeps ~50ms via the registered executor, then doubles.
    pub fn slow_double(&self, x: i32) -> i32 {
        let x_local = x;
        block_on_async(async move {
            sleep(Duration::from_millis(50)).await;
            x_local * 2
        })
        .expect("custom executor should be registered")
    }

    /// Synchronous subtract.
    pub fn subtract(&self, a: i32, b: i32) -> i32 {
        a - b
    }
}

/// A minimal `AsyncExecutor` backed by `futures::executor::block_on`.
struct FuturesExecutor;

impl AsyncExecutor for FuturesExecutor {
    fn block_on_dyn(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send>>,
    ) -> Box<dyn std::any::Any + Send> {
        // We must call `block_on` to drive the future; the boxed `()` is
        // just the type-erased return.
        #[allow(clippy::let_unit_value)]
        let _ = futures::executor::block_on(future);
        Box::new(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tokitai Runtime-Agnostic Executor Example ===\n");

    // Register our custom executor before invoking anything that needs it.
    set_async_executor(Box::new(FuturesExecutor));
    assert!(
        current_async_executor().is_some(),
        "executor should be registered"
    );

    let calc = Calculator;

    let r = calc.call_tool("add", &json!({"a": 3, "b": 4}))?;
    println!("add(3, 4)        = {}", r);

    let t0 = std::time::Instant::now();
    let r = calc.call_tool("slow_double", &json!({"x": 21}))?;
    println!("slow_double(21)  = {}  (took {:?})", r, t0.elapsed());

    let r = calc.call_tool("subtract", &json!({"a": 10, "b": 3}))?;
    println!("subtract(10, 3)  = {}", r);

    assert_eq!(r, json!(7));
    println!("\nsuccess: custom FuturesExecutor drove async tool bodies.");
    Ok(())
}
