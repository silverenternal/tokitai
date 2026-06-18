# Changelog — `tokitai-core`

All notable changes to `tokitai-core` are documented in this file. The
top-level [`tokitai/CHANGELOG.md`](../tokitai/CHANGELOG.md) lists the
unified release notes for the whole workspace; this file calls out the
`tokitai-core`-specific entries.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`ToolDefinition::with_description_explicit()`** — builder method that marks a tool description as having been supplied at compile time. Once set, `ToolDefinition::apply_configs` will NOT overwrite the description from a runtime `ToolConfig::Desc`.
- **`config::ConfigLayer` enum + `config::CONFIG_PRIORITY_ORDER` const array** — frozen priority table for the four description sources (`#[tool(desc)]`, doc comment, `tokitai!` config, synthesized default). `config_priority_table_md()` renders the table as Markdown; `can_override(compile_time_winner, runtime_layer)` returns whether a runtime override is allowed.

### Changed

- **`ToolDefinition` carries a new `description_explicit: bool` field** (T-002). Defaults to `false`; set by the `#[tool]` macro when `#[tool(desc = "...")]` is supplied.
- **`ToolDefinition::apply_configs` skips `ToolConfig::Desc` overrides** when `description_explicit == true`. Doc-comment and synthesized-default descriptions remain overridable.

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
