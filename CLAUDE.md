# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

Tokitai is a Rust workspace of four crates plus an `examples` workspace member. It turns Rust methods into AI-callable tools with a single `#[tool]` attribute. Tool definitions (name, description, JSON-Schema for params) are generated at compile time; the runtime dispatch path is in-process — no IPC, no `serde_json::Value` round-trip per call. Network transports (MCP, HTTP) are out-of-process wrappers on top of that core.

## Workspace layout

```
Cargo.toml                          # workspace root + release-profile overrides
tokitai-core/                       # zero-dep (or serde-only) types: ToolDefinition,
                                    # ToolProvider, ToolCaller, FromJsonValue, ToolError,
                                    # ParamType, ToolParameter, AsyncExecutor + set_async_executor,
                                    # ToolConfig / ToolConfigRegistry, json_schema! macro
tokitai/                            # user-facing crate: re-exports core types + the
                                    # #[tool] / #[tool_type] / #[param_tool] / #[config]
                                    # proc-macros, plus AiToolError and the optional `mcp` module
tokitai-macros/                     # proc-macro crate; all logic lives in
                                    # src/tool/mod.rs (398 lines) which glues the submodules:
                                    # attrs/, codegen/, config/, extract/, schema/, types/,
                                    # delegate/, resilience/, wrap/, wrap_openapi/
tokitai-mcp-server/                 # axum-based HTTP MCP server; server.rs is 852 lines
                                    # (McpServerBuilder, McpServer, MultiToolProvider, ServerError)
examples/                           # basic_usage, multi_tool_chat, mcp_http_server,
                                    # mcp_server_demo, dev_assistant, param_attrs,
                                    # validate_transform_alias, advanced_types,
                                    # runtime_agnostic, debug_tools, ollama_integration;
                                    # sub-crates database_tool/, starter_project/, py/, js/, go/, curl/
examples/deprecated/                # placeholder files for wrap/delegate/resilient — not
                                    # yet exposed in 0.5.0
scripts/                            # measure-consumer-impact.sh (compile-time cost harness)
docs/                               # ARCHITECTURE, wrap-architecture, MCP_ARCHITECTURE,
                                    # wrap-cheatsheet, USAGE, ADVANCED_USAGE, AI_INTEGRATION,
                                    # CROSS_LANGUAGE, performance, faq, best-practices,
                                    # quickstart, API_STABILITY, migration/, reference/, adr/, internal/
```

Crate dependency direction: `tokitai-macros` and `tokitai-core` have no deps on each other; `tokitai` depends on both; `tokitai-mcp-server` depends on `tokitai` with features `["mcp","http-server"]` and on `tokitai-core`.

## Build, lint, test

The full matrix that CI runs (`ubuntu-latest` / `macos-latest` / `windows-latest`, Rust stable) is:

```bash
# Lint + format
cargo fmt --all -- --check
cargo clippy --workspace --all-features --all-targets -- -D warnings

# Build (both ends of the feature spectrum)
cargo build --workspace --all-features
cargo build --workspace --no-default-features

# Test
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo test --doc                                         # doctests in every crate

# Doc build (CI uses RUSTDOCFLAGS=-D warnings)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

Targeted / single-test commands:

```bash
# One crate, all tests
cargo test -p tokitai-core
cargo test -p tokitai
cargo test -p tokitai-macros
cargo test -p tokitai-mcp-server

# Single test by name (substring match)
cargo test -p tokitai-macros test_basic_tool
cargo test -p tokitai-core  test_async_executor_lifecycle

# trybuild UI tests live in tokitai-macros/tests/ui_tests.rs and point at
# tests/ui/*.rs / tests/ui/*.stderr. To refresh a snapshot after an intentional
# compiler-message change:
TRYBUILD=overwrite cargo test -p tokitai-macros --test ui_tests test_generic_method

# Property-based test (has its own snapshot fixture; see notes below)
BLESS=1 cargo test -p tokitai-macros --test property_based_test snapshot_5_method_fixture

# Macro compile-time benchmark (criterion)
cargo bench -p tokitai-macros --bench macro_bench

# Run an example / fixture
cargo run --example basic_usage
cargo run -p tokitai-mcp-server --example mcp_builder_demo   # binds 127.0.0.1:8080
cargo run --example dev_assistant                            # end-to-end regression test
```

## How `#[tool]` actually works (the part that surprises newcomers)

1. `#[tool]` on an `impl` block parses the block, walks every `pub` method, and emits three artifacts:
   - `fn __get_tool_definitions() -> &'static [ToolDefinition]` — backed by a `LazyLock<Vec<ToolDefinition>>` so each entry is built once per process.
   - `impl ToolProvider for <Self>` returning that slice.
   - `pub fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError>` — a `match` over tool names that delegates to a per-method `__call_<name>` wrapper.
2. Per-parameter parsing is generated from each Rust parameter's type via `FromJsonValue` (defined in `tokitai-core`); custom structs deserialize via `serde_json::from_value` at the call site.
3. **Async from a sync caller.** When a tool method is `async fn`, the macro generates a sync wrapper that drives the future. The wrapper probes three paths in order: a user-registered `AsyncExecutor` (set once via `tokitai_core::set_async_executor(Box::new(my_exec))`), then the current Tokio runtime via `Handle::block_on`, then `block_on_async_error_message()`. Without a Tokio runtime **and** without a registered executor, the call returns `ToolError::InternalError("no async runtime registered; …")`. For non-Tokio runtimes (`async-std`, `smol`), the user MUST register a custom executor — the Tokio fallback won't apply.
4. **Wrap features** (`#[wrap]`, `#[openapi]` / `#[openapi_op]`, `#[delegate]`, `#[retry]`, `#[rate_limit]`, `#[circuit_breaker]`) are documented in `docs/wrap-architecture.md` and `docs/wrap-cheatsheet.md`. As of 0.5.0 the dedicated examples for `wrap_native`, `delegate_method`, and `resilient_tool` live in `examples/deprecated/` — the corresponding attributes are documented but not all are exposed via runnable examples yet. `#[openapi]` does have a runnable example (`examples/wrap_openapi.rs`).
5. The tool-definition schema is **compile-time only** — `input_schema` is a `&'static str` literal baked into the binary. Runtime mutations go through `ToolConfig` / `ToolConfigRegistry` / `GLOBAL_CONFIG_REGISTRY` (in `tokitai-core/src/config.rs`) and `apply_configs` on `ToolDefinition`.

## Conventions enforced by the build / CI

- **Edition 2021, MSRV 1.80.** Do not use 1.70+-only idioms in `Cargo.toml` feature lists.
- **Release profile** is set at the workspace root: `lto = "thin"`, `codegen-units = 1`, `strip = "debuginfo"`. The `tokitai-macros` package additionally gets `opt-level = 3` because macro expansion is CPU-bound.
- **Macro warnings are silenced in test builds** via `tokitai-macros/build.rs`, which sets `TOKITAI_QUIET=1`. End-user crates that want warnings should set `TOKITAI_SHOW_WARNINGS=1` themselves; the user-facing crate `tokitai` re-exports a `TOKITAI_QUIET` knob.
- **`#[deny(missing_docs)]` is on for `tokitai-core`** (see its `lib.rs`). New public items need doc comments there.
- **Clippy is `-D warnings`** for the whole workspace under `--all-features --all-targets`.
- **`cargo doc` is `-D warnings` under `RUSTDOCFLAGS`** — broken doc links break CI.
- **Commit messages follow Conventional Commits** (`feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`); recent history shows the project also uses `fix(ci)` and `test(ui)` scopes.
- **Dependabot** opens weekly cargo PRs grouped into minor+patch (max 5 open). Major bumps for `axum` and `tokio` are intentionally ignored — they ship breaking changes that the build matrix doesn't always catch.
- **Every PR** runs the same checklist (from `.github/pull_request_template.md`): `cargo fmt`, `cargo clippy` (with all features + all targets, `-D warnings`), `cargo test --workspace --all-features`, `cargo doc` (with `-D warnings`). New behavior needs new tests; CHANGELOG.md gets an `[Unreleased]` entry.

## Things that will burn you if you don't know

- **UI tests are trybuild-driven.** Source fixtures live at `tokitai-macros/tests/ui/*.rs`; expected stderr at `tokitai-macros/tests/ui/*.stderr`. Rustc version changes the wording — a CI failure on `08_generic_method` after bumping rustc almost always means a snapshot refresh, not a regression.
- **`examples/deprecated/` is intentional.** Those `.rs` files are placeholders showing what `#[wrap]` / `#[delegate]` / `#[retry]` / `#[rate_limit]` / `#[circuit_breaker]` will look like once exposed; do not delete them and do not move them back into `examples/`.
- **`Cargo.lock` exists at both the workspace root and inside `tokitai/` / `tokitai-macros/`.** They're not in sync; the workspace root is canonical.
- **`AiToolError` (in `tokitai/src/error.rs`) and `ToolError` (in `tokitai-core`)** are two different error types. `AiToolError` is a richer runtime wrapper around `ToolError`. Don't conflate them.
- **The `phf::Map` for `#[openapi]` is baked at compile time** from the spec file the user names. There is no runtime spec reload.
- **Wrap and resilience decorators are composable, but outer-wins**: stacked `#[retry(max=3)] #[rate_limit(rps=10)]` means rate-limit is the outer guard, retry is the inner loop.