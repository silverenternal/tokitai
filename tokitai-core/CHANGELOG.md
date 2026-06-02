# Changelog — `tokitai-core`

All notable changes to `tokitai-core` are documented in this file. The
top-level [`tokitai/CHANGELOG.md`](../tokitai/CHANGELOG.md) lists the
unified release notes for the whole workspace; this file calls out the
`tokitai-core`-specific entries.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-06-02

### Added

- **`ToolDefinition::to_openai_function()`** — convert any `ToolDefinition` to the OpenAI function-calling JSON envelope (`{ "type": "function", "function": { … } }`).
- **`ToolDefinition::to_anthropic_tool()`** — convert any `ToolDefinition` to the Anthropic tool-use JSON envelope (`{ name, description, input_schema }`).
- **`ToolDefinition::to_mcp_tool()`** — convert any `ToolDefinition` to the Model Context Protocol (MCP) tool definition JSON envelope (`{ name, description, inputSchema }`).
- **`tokitai_core::AsyncExecutor` trait** — runtime-agnostic async executor abstraction. Replaces the previous hard coupling to `tokio::runtime::Handle::block_on`. Includes `set_async_executor`, `current_async_executor`, and `block_on_async` API. See `examples/runtime_agnostic.rs` and `tokitai-core/tests/async_executor_no_executor_test.rs`.

### Changed

- **`AsyncExecutor` is now object-safe** — was generic over `F: Future`; the new signature uses `Pin<Box<dyn Future + Send>>` for the future parameter and `Box<dyn Any + Send>` for the return. This is a **breaking change** to the trait; users who implemented it manually in 0.4.0 must update to the new signature (see migration note in the top-level CHANGELOG).

### Fixed

- **Removed duplicate `[features]` key in `Cargo.toml`** that was causing `error: duplicate key` on `cargo build`.

## [0.4.0] - 2025-XX-XX

- Initial public release of `tokitai-core`. Provides the zero-dependency core types (`ToolDefinition`, `ToolProvider`, `ToolCaller`, `ToolError`) and the first version of the `AsyncExecutor` trait (later made object-safe in 0.5.0).
