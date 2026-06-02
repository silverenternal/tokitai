# Changelog

All notable changes to Tokitai will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-06-02

### Added

- **`#[wrap]` proc-macro attribute** — signature-driven auto-wrapping of a third-party client. Place `#[wrap(client = MyClient, methods = [foo, bar])]` on an `impl` block to generate the `ToolProvider` impl from the listed methods, plus a `new(client)` constructor. See `examples/wrap_native.rs` and `tests/wrap_native_test.rs`.
- **`#[openapi]` and `#[openapi_op]` proc-macro attributes** — OpenAPI 3.0 spec-driven wrapper. The spec is read with `include_str!` and parsed at proc-macro compile time; operations are baked into a `phf::Map<&str, __OpenApiOp_*>` keyed by `operationId` for O(1) runtime lookup. See `examples/wrap_openapi.rs` and `tests/wrap_openapi_test.rs`.
- **`#[delegate]` proc-macro attribute** — method-level transparent forwarding. Annotate a signature with `#[delegate(to = "self.inner")]` and the macro injects a body that evaluates `<to>.<method_name>(<args>)` (with `.await` for `async fn`). See `examples/delegate_method.rs` and `tests/delegate_method_test.rs`.
- **`#[retry]` proc-macro attribute** — retry decorator with `max` (default `3`), `backoff` (`"constant"`, `"linear"`, `"exponential"`; default exponential), `jitter` (default `true`), and `on` (error filter; accepted for forward compatibility, v1 retries on any `Err`). Works on sync and async methods; async sleeps are driven through the registered `tokitai_core::AsyncExecutor`. See `examples/resilient_tool.rs` and `tests/resilience_test.rs`.
- **`#[rate_limit]` proc-macro attribute** — token-bucket rate limiting with `rps` (default `1`) and `burst` (default `1`) options. Lock-free implementation built on two `AtomicU32`/`AtomicU64` and a single 32-bit CAS per call. See `examples/resilient_tool.rs` and `tests/resilience_test.rs`.
- **`#[circuit_breaker]` proc-macro attribute** — three-state (closed / open / half-open) circuit breaker with `failure_threshold` (default `5`) and `reset_timeout` (default `"30s"`) options. State is held in three static atomics per decorated function. v1 is observe-only and does not fail-fast. See `examples/resilient_tool.rs` and `tests/resilience_test.rs`.
- **`ToolDefinition::to_openai_function()`** — convert any `ToolDefinition` to the OpenAI function-calling JSON envelope (`{ "type": "function", "function": { … } }`). (`tokitai-core`)
- **`ToolDefinition::to_anthropic_tool()`** — convert any `ToolDefinition` to the Anthropic tool-use JSON envelope (`{ name, description, input_schema }`). (`tokitai-core`)
- **`ToolDefinition::to_mcp_tool()`** — convert any `ToolDefinition` to the Model Context Protocol (MCP) tool definition JSON envelope (`{ name, description, inputSchema }`). (`tokitai-core`)
- **`tokitai_core::AsyncExecutor` trait** — runtime-agnostic async executor abstraction. Replaces the previous hard coupling to `tokio::runtime::Handle::block_on`. Users on `async-std` / `smol` / custom runtimes can register their own executor at program startup. Includes `set_async_executor`, `current_async_executor`, and `block_on_async` API. See `examples/runtime_agnostic.rs` and `tokitai-core/tests/async_executor_no_executor_test.rs`.
- **`docs/wrap-architecture.md`** — comprehensive architecture documentation covering all 5 wrap features (composition rules, performance characteristics, limitations, future work).
- **`docs/wrap-cheatsheet.md`** — 1-page quick reference for the 5 wrap features.

### Changed

- **`tokitai-core`: `AsyncExecutor` is now object-safe** — was generic over `F: Future`; the new signature uses `Pin<Box<dyn Future + Send>>` for the future parameter and `Box<dyn Any + Send>` for the return, enabling `dyn AsyncExecutor` storage and registration.

### Fixed

- **`#[tool]` macro: sync-from-async lifetime bound** — `__call_*_sync` wrapper now compiles for methods that borrow `&self` with non-`'static` lifetime. The fix routes the body through `block_on_dyn` (type-erased `Send` future) instead of `block_on_async` (which required `F: 'static`).
- **`#[tool]` macro: duplicate `__call_*` definitions for mixed sync/async methods** — an `impl` block that contained both sync and async methods generated two `__call_X` functions that collided at link time. The fix renames the second to `__call_X_async` (or similar suffix) and updates the dispatcher accordingly.
- **i18n: hard-coded Chinese error messages in macro-generated wrappers** — replaced with English defaults, with an i18n hook that reads `LANG` / `LC_ALL` environment variables at compile time to localize error text.
- **`tokitai-core`: removed duplicate `[features]` key in `Cargo.toml`** that was causing `error: duplicate key` on `cargo build`.

### Migration from 0.4.0

The 0.5.0 release is fully backwards-compatible for users of `#[tool]`. The new
`#[wrap]`, `#[openapi]`, `#[delegate]`, `#[retry]`, `#[rate_limit]`, and
`#[circuit_breaker]` attributes are purely additive — old code compiles and
behaves identically. The new `ToolDefinition::to_openai_function()`,
`to_anthropic_tool()`, and `to_mcp_tool()` methods are also additive.

The only breaking change is to `tokitai_core::AsyncExecutor`, which was
introduced in 0.4.0 and is rarely used directly. If you implemented it
manually (e.g. to plug in `async-std` or `smol`), update to the new
object-safe signature: replace the generic `F: Future` parameter with
`Pin<Box<dyn Future + Send>>` and return `Box<dyn Any + Send>`.

```rust
// 0.4.0
impl AsyncExecutor for MyExecutor {
    fn block_on<F: Future>(&self, fut: F) -> F::Output { … }
}

// 0.5.0
impl AsyncExecutor for MyExecutor {
    fn block_on(&self, fut: Pin<Box<dyn Future + Send>>) -> Box<dyn Any + Send> { … }
}
```

If you did not implement `AsyncExecutor` yourself, no action is required —
the default tokio-backed implementation is updated automatically.

## [0.4.0] - 2025-XX-XX

- Initial public release of Tokitai on crates.io.
- Introduced the `#[tool]` proc-macro attribute: place it on an `impl` block to generate the `ToolProvider` / `ToolCaller` impls and the `call_tool` dispatcher from your public methods at compile time. Rust types are mapped to JSON Schema; tool definitions are emitted as `&'static [ToolDefinition]`.
- Introduced the optional `mcp` and `http-server` features for serving the same tool definitions over the Model Context Protocol (HTTP+JSON). Includes `tokitai_core` (zero-dependency core types) and `tokitai-macros` (the proc-macro crate).
