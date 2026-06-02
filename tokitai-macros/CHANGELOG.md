# Changelog — `tokitai-macros`

All notable changes to `tokitai-macros` are documented in this file. The
top-level [`tokitai/CHANGELOG.md`](../tokitai/CHANGELOG.md) lists the
unified release notes for the whole workspace; this file calls out the
`tokitai-macros`-specific entries.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-06-02

### Added

- **`#[wrap]` proc-macro attribute** — signature-driven auto-wrapping of a third-party client. Generates the `ToolProvider` impl and a `new(client)` constructor from a curated method list. See `examples/wrap_native.rs` and `tests/wrap_native_test.rs`.
- **`#[openapi]` and `#[openapi_op]` proc-macro attributes** — OpenAPI 3.0 spec-driven wrapper. The spec is parsed at proc-macro compile time and operations are baked into a `phf::Map` for O(1) runtime lookup. See `examples/wrap_openapi.rs` and `tests/wrap_openapi_test.rs`.
- **`#[delegate]` proc-macro attribute** — method-level transparent forwarding. The macro injects the forwarded body from a `to = "..."` expression; no body is required. See `examples/delegate_method.rs` and `tests/delegate_method_test.rs`.
- **`#[retry]` proc-macro attribute** — retry decorator with `max`, `backoff` (`constant` / `linear` / `exponential`), `jitter`, and `on` options. Works on sync and async. See `examples/resilient_tool.rs` and `tests/resilience_test.rs`.
- **`#[rate_limit]` proc-macro attribute** — token-bucket rate limiting with `rps` and `burst` options. See `examples/resilient_tool.rs` and `tests/resilience_test.rs`.
- **`#[circuit_breaker]` proc-macro attribute** — three-state circuit breaker with `failure_threshold` and `reset_timeout` options. See `examples/resilient_tool.rs` and `tests/resilience_test.rs`.

### Fixed

- **`#[tool]` macro: sync-from-async lifetime bound** — `__call_*_sync` wrapper now compiles for methods that borrow `&self` with non-`'static` lifetime. Routed through `block_on_dyn` instead of `block_on_async`.
- **`#[tool]` macro: duplicate `__call_*` definitions for mixed sync/async methods** — second definition is now renamed (e.g. `__call_X_async`) and the dispatcher is updated accordingly.
- **i18n: hard-coded Chinese error messages in macro-generated wrappers** — replaced with English defaults + i18n hook (reads `LANG` / `LC_ALL` at compile time).

## [0.4.0] - 2025-XX-XX

- Initial public release of `tokitai-macros`. Provides the `#[tool]` proc-macro attribute that turns the public methods of an `impl` block into compile-time tool definitions and a `call_tool` dispatcher.
