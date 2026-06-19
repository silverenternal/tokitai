# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Adversarial description lint (T-022, PP-H1).** `#[tool(desc = "...")]` literals are checked at macro-expansion time against a bad-pattern bitmask set covering instruction-like phrases (`"ignore previous"`, `"ignore all"`, `"always respond"`, `"you must"`, `"do not mention"`), role headers (`"system:"`, `"assistant:"`, `"user:"`), fake-prompt multi-line blocks (3+ consecutive newlines), oversized narratives (>2000 chars), and non-ASCII bytes (Unicode homoglyph bypass). The same check is mirrored in `tokitai-mcp-server`'s `TypedDispatcher::tools_list` so injected descriptions that slip past the macro are caught at server start. Users can extend the bad-pattern set with `#[tool(desc_blocklist = ["..."])]` or the per-build `TOKITAI_DESC_BLOCKLIST` env var, and can opt out per-method with `#[tool(allow_insecure_desc)]`. `docs/AI_INTEGRATION.md` gains a "Defending the description channel" section anchored on the Tencent Cloud 2026-06-19 and CSDN 2026-06-07 threat-model reports.

- **Per-tool capability manifest (T-023, PP-H2).** `#[tool(requires = ["db:read:sales"])]` declares a tool's blast radius. The macro emits per-method `CAPABILITIES_<NAME>` constants and an aggregated `CAPABILITIES` slice. The MCP server's `McpServerBuilder::with_capability_allowlist(vec![...])` refuses to bind if any tool's required capabilities are not in the operator-supplied allowlist (fail-closed when set). Wildcard entries (`"db:read:*"`) match via trailing-`*` prefix; bare `"*"` matches everything. The `CapabilityManifestProvider` trait is a backwards-compatible supertrait addition. `docs/MCP_ARCHITECTURE.md` gains a "Capability model (T-023)" section.

- **Cross-crate version assertion (T-024, PP-H3).** `tokitai_core::assert_compatible_with("0.5")` is a `pub const fn` that panics on mismatch (invoked from a `const _: () = ...` block in the consumer crate, the panic reaches `compile_error!`). `tokitai-mcp-server::serve()` reads `--require-tokitai=<prefix>` from CLI args and compares it against the manifest version (baked at compile time via `build.rs` from the workspace `Cargo.lock`). On mismatch the server returns `Err(ServerStartupError)` so the caller can exit 78 (`EX_CONFIG`). A `--allow-tokitai-mismatch` flag logs at `warn!` for the audit trail. `docs/API_STABILITY.md` gains a "Cross-crate version assertion (T-024)" operator runbook.

- **Compile-time schema-dialect correctness (T-012, DS-1).** The `#[tool]` macro now accepts an impl-block `dialect = "..."` attribute (`"mcp"`, `"openai-strict"`, `"anthropic"`) and audits every emitted `ToolDefinition.input_schema` against the chosen provider's known quirks. Violations surface as `compile_error!` invocations anchored at the user-written method span (T-001). Default dialect is `mcp` (loosest); choosing a stricter dialect without an emitted workaround is a compile error, not a warning. The rule set is centralised in `tokitai-macros/src/tool/schema/dialect.rs` with stable codes (`MCP-1`, `OA-1`, `OA-2`, `OA-3`, `AN-1`). New `tokitai-macros/tests/dialect_audit_test.rs` covers four positive cases (each dialect + alias names) and five negative cases live as trybuild fixtures under `tests/ui/15_unknown_dialect.rs` through `tests/ui/19_mcp_missing_type.rs`. `docs/wrap-architecture.md` gains a "Dialect correctness" section (§8); the top-level README advertises this as the answer to "why does my tool fail in OpenAI but work in Claude?".

- **Offline-compatible LLM provider-envelope round-trip test (T-006, PP-D2).** New `tokitai/tests/provider_envelope_test.rs` round-trips a `ToolDefinition` through OpenAI's `function.parameters`, Anthropic's `input_schema`, and MCP's `inputSchema` envelopes via `to_openai_function` / `to_anthropic_tool` / `to_mcp_tool`. Recorded JSON request fixtures (`tests/fixtures/openai_chat_completion_request.json`, `tests/fixtures/anthropic_messages_request.json`, `tests/fixtures/mcp_tools_list_response.json`) anchor the envelope shape and the test asserts every property and required key survives the round-trip. A new optional `llm-live` feature (off by default; not in CI) enables a real round-trip against a local Ollama instance when `OLLAMA_HOST` is set. The macro-generated `tool_definitions()` are exercised through the envelope emitters to prove no field is dropped between `#[tool]` and the wire.

- **First-class tool versioning (T-013, DS-2).** `tokitai_core::ToolDefinition` already carried `version`, `deprecated_since`, `remove_in`, and `replaced_by` fields; T-013 wires them through the macro and the runtime so deprecation is a type-system concern, not a hand-rolled docstring. The `#[tool]` macro now accepts the deprecation triple `#[tool(deprecated_since = "...", remove_in = "...", replaced_by = "...")]` plus `#[tool(version = "...")]`, surfaces the lifecycle fields on the stored `ToolDefinition` and on the provider envelopes (`to_openai_function` / `to_anthropic_tool` / `to_mcp_tool` — the MCP envelope gains a structured `_meta` block with `deprecated`, `deprecatedSince`, `removeIn`, `replacedBy`). A new `ToolErrorKind::Removed` variant and `ToolError::removed(...)` constructor surface the lifecycle failure to callers. The dispatcher refuses to call a tool whose `remove_in` is at or before the program-wide current version (set once at startup via the new `tokitai::set_current_version("X.Y.Z")`); the resulting `ToolError::Removed` message names the tool, the boundary version, and the `replaced_by` successor. A new `replaced_by` redirect in the dispatcher's fallback arm routes calls for removed tool names to the successor (one hop, no loop). New `tokitai/tests/tool_versioning_test.rs` exercises the full lifecycle (current → deprecated → removed → `replaced_by` redirect). `docs/API_STABILITY.md` now describes the deprecation lifecycle end-to-end.

- **First-class `set_runtime_executor` API with per-call override (T-003, PP-C1).** `tokitai_core::AsyncExecutor` gains a `block_on_for(&self) -> Option<&'static dyn AsyncExecutor>` trait method (default `None`). A new free helper `tokitai_core::block_on_for_executor()` exposes the per-call probe so the `#[tool]` macro and the resilience decorators (`#[retry]`, `#[rate_limit]`, `#[circuit_breaker]`) can resolve an executor through this seam before falling back to the global slot set by `set_async_executor`. The `set_async_executor` rustdoc now explicitly calls out `async-std`, `smol`, and `embassy` as supported use cases. The macro's sync-from-async wrapper probes `block_on_for_executor()` and `current_async_executor()` *before* the active Tokio runtime and surfaces a clear English error explaining the `&self`-vs-`'static` constraint when a non-Tokio executor is registered but no Tokio runtime is in scope. New `tokitai-core/tests/async_executor_override_test.rs` exercises the seam end-to-end with an `async-std`-style executor stub and proves (via internal counters) that the override path was actually taken, not the global fallback.

- **Frozen configuration priority table (T-002, PP-B2).** Tool descriptions supplied via `#[tool(desc = "...")]` are now marked `description_explicit: true` on the generated `ToolDefinition` and the runtime `tokitai!` configuration block will NOT override them. Doc-comment and synthesized-default descriptions stay open to runtime override. The priority is exposed as a `const fn` table at `tokitai_core::config::CONFIG_PRIORITY_ORDER` (with the markdown renderer `config_priority_table_md()` and the boolean helper `can_override`), so the macro, the user-facing docs, and the test suite cannot drift. New `tokitai/tests/config_override_test.rs` pins the behaviour; `docs/USAGE.md` and `docs/ADVANCED_USAGE.md` now contain the same priority table lifted from that constant.

- **Per-impl-block compile-time profiling (T-011, PP-A3).** The `#[tool]` macro now emits a `cargo:warning=impl <TYPE> -> <TOOLS> tools, ms=<MICROS>` line for every `#[tool]` impl block when `TOKITAI_PROFILE=1` is set in the build environment. The cost is gated behind `option_env!("TOKITAI_PROFILE")` so default builds pay nothing — the macro reads the env var via `tokitai-macros/build.rs`, which forwards the value as `cargo:rustc-env=TOKITAI_PROFILE=...`. `scripts/measure-consumer-impact.sh` reads the warnings and prints a per-impl profile report (median µs per impl block) when invoked with `TOKITAI_PROFILE=1 bash scripts/...`. CI captures the per-impl median as the `per-impl-profile` artifact (new `per-impl-profile` job in `.github/workflows/ci.yml`); once enough green runs establish a baseline, a >20% median regression will fail the build. New `tokitai-macros/tests/per_impl_profile_test.rs` pins the warning format (`cargo:warning=impl ` prefix, ` -> ` separator, ` tools, ms=<number>` tail), confirms the env-var gate lives in `tokitai-macros/src/tool/mod.rs` (so the default path skips the timing path), and confirms `build.rs` forwards the env var via `cargo:rustc-env`. `docs/performance.md` §"Per-impl profile mode (T-011)" documents the user-facing workflow.

- **Per-impl-block token-cost warnings for tool schemas (T-014, DS-3).** Companion to the T-011 profile warning. When `TOKITAI_PROFILE=1` is set, the `#[tool]` macro now also emits a `cargo:warning=impl <TYPE> -> <TOOLS> tools, schema_bytes=<B>, est_tokens=<T>` line per impl block, where `<B>` is the byte count of every `name + description + input_schema` string the impl will emit (using a 4×-description proxy for the schema body), and `<T>` is `ceil(B/4)`. A second env var `TOKITAI_PROFILE_BUDGET=<N>` enables a per-impl budget gate: when `schema_bytes` exceeds `<N>`, the macro emits an additional `cargo:warning=impl <TYPE> -> <TOOLS> tools, schema_bytes=<B> exceeds budget=<N>` line (build continues; budget is a hint). Both warnings are gated behind `option_env!` so default builds pay zero cost. `tokitai-macros/build.rs` forwards both env vars via `cargo:rustc-env` so the macro can read them via `option_env!`. New `examples/budget_check.rs` defines a 200-method `BigTools` impl + 3-method `SmallTools` impl so CI exercises both the warning-fires (BigTools) and warning-stays-quiet (SmallTools) paths. New `tokitai-macros/tests/token_cost_warning_test.rs` pins the format, the `div_ceil(4)` heuristic, the env-var forward in `build.rs`, and the gating contract (default build pays no byte walk). CI gains two new jobs: `budget-check` builds the example with `TOKITAI_PROFILE_BUDGET=8192` and asserts BigTools fires + SmallTools stays quiet; `token-cost-baseline` builds the workspace with `TOKITAI_PROFILE=1` and captures every per-impl `schema_bytes` + `est_tokens` line as the `per-impl-token-cost` artifact. `scripts/measure-consumer-impact.sh` gains `parse_token_cost_warnings` and `parse_budget_warnings` helpers and a "per-impl token-cost report" section. `docs/performance.md` §"Token cost of tool schemas (T-014)" walks through the format, the 4× proxy heuristic, and the budget-gate remediation paths.

- **Typed MCP handle layer (T-021, PP-G5).** Defense-in-depth against the CVE-2025-59377 class of MCP vulnerabilities (e.g. `mcp-kubernetes-server` 0.1.11 OS command injection via `subprocess.run(shell=True)` after the JSON schema was supposed to validate). New `tokitai-mcp-server/src/typed.rs` exposes `validate_against_schema`, `validate_tool_args`, and `TypedDispatcher` — a runtime JSON-Schema validator that walks every tool's `inputSchema` (loaded from `tests/fixtures/mcp-spec/typed/*.json`) and refuses malformed calls with `ToolError::ValidationError` BEFORE the handler is invoked. The error message includes an RFC 6901 JSON Pointer to the offending field. The typed layer is opt-in via the new `mcp-typed` Cargo feature in `tokitai-mcp-server/Cargo.toml` (off by default; default build behavior is byte-identical to T-005). Hard rule reaffirmed: NO dependency on `rmcp` or any MCP SDK is added; `grep -i rmcp tokitai-mcp-server/Cargo.toml` returns nothing. New `tokitai-mcp-server/tests/mcp_typed_layer_test.rs` covers 5 positive, 7 negative, and 4 fuzz (100 random shapes/tool) cases plus 3 validator-only checks. `docs/MCP_ARCHITECTURE.md` gains the "Typed handle layer (T-021)" section with the threat-model table, the CVE-2025-59377 mapping, the feature-gate summary, and the public API example.

- `ToolDefinition::apply_configs` now skips `ToolConfig::Desc` overrides when `description_explicit == true`. This is a behavioural change for the rare case where both an explicit `#[tool(desc)]` attribute AND a `tokitai!` runtime `desc:` were supplied; the compile-time attribute wins.

### Security

- **Fail-closed typed MCP handle layer (T-021 hardening).** `TypedDispatcher::handle_call` now refuses calls for tools that have no typed spec (warn-once per tool name, then `ToolError::ValidationError`). The typed validator is fail-closed against all JSON-Schema subset features: type mismatches, missing required properties, `additionalProperties: false`, numeric/string bounds, integer-arity i64 arithmetic. Unknown keywords reject the schema at server start.

- **Adversarial description lint Unicode bypass fix (C-3).** The T-022 matcher now rejects any non-ASCII byte (bytes outside `0x20-0x7E` plus `0x09`) at the matcher boundary, preventing Cyrillic-homoglyph bypasses of the instruction-phrase and role-header blocklist.

- **TOKITAI_VERSION_OVERRIDE spoofing path removed (C-2).** The build-time override that silently forged the T-024 version constant has been removed. Setting the env var now emits a `cargo:warning=` with no effect on the generated manifest. Canary/staging deploys must use a release branch.

### Fixed

- **parse_semver UB on >3 components (C-1).** Both `tokitai_core::parse_semver_const` and `tokitai_mcp_server::parse_semver_tuple` now return `None` on 4+ component SemVer inputs (e.g. `"0.0.0.0"`) instead of panicking with an index-out-of-bounds at the `parts[arity]` write site.

- **build.rs Cargo.lock path resolution (M-3).** `tokitai-mcp-server/build.rs` now walks upward from `CARGO_MANIFEST_DIR` looking for a `Cargo.toml` with `[workspace]`, instead of hardcoding `../Cargo.lock`. When the resolved version is empty, a `cargo:warning=` is emitted and the sentinel `0.0.0-unresolved` is written so the runtime refusal trips loudly. Adds `workspace_root_contains_cargo_lock_with_tokitai_core` integration test.

- **`--require-tokitai=` empty prefix rejection (M-5/M-9).** `parse_serve_args` now returns `Result<ServeArgs, &'static str>` and rejects an empty or whitespace-only `--require-tokitai=` prefix as an operator error, instead of silently treating it as "no requirement" (which disabled the version check behind a typo).

- **T-025: adversarial description lint follow-ups.** `tokitai-macros/src/tool/attrs/method.rs` and `desc.rs` now share a single `desc_blocklist::parse_phrase_array` helper (was duplicated between the macro and the runtime mirror), the `M-2` fixture at `examples/w022-desc-blocklist.rs` is the new home for the blocklist parity test, and the `docs/MCP_ARCHITECTURE.md` "Typed handle layer" section now cross-references the `T-022` "Defending the description channel" threat model so the macro-side and server-side matchers cannot drift again.

- **T-026: capability-manifest hardening folded into T-028.** The `CapabilityManifestProvider` `Display` impl uses a names allowlist (only the three `CapabilitySegment` variants render through the public formatter; any future variant is forced to opt in), parse-error spans point at the offending `requires = [...]` entry (not the whole impl block), the `W023 missing_capabilities` message cross-references any other `allow = [...]` use in the same impl block, an empty `CAPABILITIES` slice allocates no `Vec` (compile-time slice literal), and the `requires_invalid` flag is rejected at parse time (was previously a silent skip).

- **T-027: `TypedDispatcher` per-tool caching and `serve()` integration test.** `TypedDispatcher` now caches the compiled `JSONSchema` for every tool spec (built once on first `validate_tool_args` call, reused on every subsequent call). The `serve()` entry point gains an end-to-end integration test that wires a real `McpServerBuilder` + `TypedDispatcher` + `MultiToolProvider` and proves a malformed call is rejected with `ToolError::ValidationError` (RFC 6901 pointer to the offending field) BEFORE the user handler is invoked, then a well-formed call round-trips through to the handler.

- **T-028: code quality and API surface cleaning.** `tokitai-core` deduplicates the orphan doc comment on `capability_in_allowlist` (CapabilityManifestProvider trait block removed), `assert_compatible_with` const-fn panics now embed the actual `CORE_VERSION` via `concat!(env!(...))` and a new `assert_compatible_with_runtime` non-const helper uses `format!` to include both expected and actual versions; `tokitai-mcp-server` adds a `///` doc on the `run_version_check` alias noting the relationship to `serve()`; `tokitai-macros` introduces the `E0033` error code (a stray `category = "..."` attribute is now rejected at parse time instead of being silently treated as a `tags.push` alias), hoists `should_show_warnings()` out of the per-method loop into the per-impl-block scope, and the `build.rs` sanity-checks `TOKITAI_DESC_BLOCKLIST` (non-ASCII bytes, oversized payloads, or phrases > 256 bytes all emit a `cargo:warning=`).

### Changed

- **`desc_blocklist` doc syntax corrected (M-7).** The documentation in `docs/AI_INTEGRATION.md` showed `desc_blocklist(["phrase"])` (function-call style). The macro's parser only accepts the key/value form `desc_blocklist = ["phrase"]`; docs now match the parser.

- **MCP_ARCHITECTURE.md W023 doc corrected.** The claim that the default-build is "intentionally noisy" was misleading — all three `build.rs` scripts set `TOKITAI_QUIET=1` by default, so `W023` is silent in first-party builds. User crates without `TOKITAI_QUIET` see the warning as documented.

## [0.5.1] - 2026-06-02

### Fixed

- **Documentation localisation pass**: All user-facing Markdown (5 READMEs, the 4 lib-level rustdoc, the long-form docs, the `CHANGELOG`, `CONTRIBUTING`, and the `examples/` files) is now in English with no emoji decoration. The v0.5.0 entry is no longer the only clean entry — every historical entry is translated too.
- **Wrapper-feature docs are now linked from both crate READMEs**: The `#[wrap]`, `#[openapi]` / `#[openapi_op]`, `#[delegate]`, `#[retry]`, `#[rate_limit]`, and `#[circuit_breaker]` attributes are documented under `docs/reference/` and the `docs/wrap-architecture.md` / `docs/wrap-cheatsheet.md` cheat sheets. Both the workspace root README and the `tokitai` sub-crate README now have a prominent "Wrap features (v0.5+)" section linking to them.
- **Version drift in rustdoc**: The `tokitai = "0.4"` example in `tokitai/src/lib.rs` and `tokitai-macros/src/lib.rs` and the `tokitai = { version = "0.3", default-features = false }` example in `tokitai/src/lib.rs` are now aligned with the current 0.5.x line.
- **`tokitai::config!` docstring example** in `tokitai-macros/src/lib.rs` was in Chinese (`"获取用户信息"` / `"用户唯一标识"`); now in English.
- **`tokitai/src/mcp.rs` module-level rustdoc** was in Chinese; now in English.

## [0.5.0] - 2026-06-02

### Breaking

- No public API break relative to 0.4.0. (`TOOL_DEFINITIONS` const → `tool_definitions()` method was already shipped in 0.4.0; see the 0.4.0 entry for the migration steps.)
- One known macro bug ("duplicate `__call_X`" on mixed sync/async impls) is fixed — affected users may simply have to delete a workaround.
- The `tokitai_core::AsyncExecutor` trait gained a `block_on_dyn(&mut self, ...)` method (the old `block_on` static helper remains for backwards compatibility).

### Added

- **Wrap feature stabilisation (X-series)** — compile-time optimisations X1–X8, integrating `#[wrap]`, `#[openapi]`, `#[openapi_op]`, `#[delegate]`, `#[retry]`, `#[rate_limit]`, and `#[circuit_breaker]` into the same codegen pipeline as `#[tool]`.
- **Resilience decorators** — `#[retry]`, `#[rate_limit]`, and `#[circuit_breaker]` can now wrap individual `#[tool]` methods; the three are described in `docs/wrap-architecture.md` and the per-macro pages under `docs/reference/`.
- **Delegate macro** — `#[delegate]` forwards an inner struct's public methods as tools on the outer impl, removing the need to hand-write `match` dispatch.
- **OpenAPI bridge** — `#[openapi]` and `#[openapi_op]` read an OpenAPI 3 spec and expose every operation (or a whitelisted subset) as a `#[tool]`.
- **Runtime-agnostic async executor** — `tokitai_core::AsyncExecutor`, `set_async_executor`, `block_on_async`, and `block_on_dyn` let you drive `#[tool]` async methods from any runtime (Tokio, `futures::executor::block_on`, an embedded executor, etc.).
- **Cross-language SDK examples** — `examples/py/`, `examples/js/`, `examples/go/`, and `examples/curl/` are runnable reference clients against `tokitai-mcp-server`. The HTTP+JSON contract is documented in `docs/CROSS_LANGUAGE.md`.
- **New end-to-end example** — `examples/dev_assistant.rs` (file/git/calc tools) is now the downstream-consumer regression test; see `BUGS_FOUND.md` for the regression net.
- **Documentation sweep (Y-series)** — added Y1–Y8 doc sections (`docs/wrap-architecture.md`, `docs/wrap-cheatsheet.md`, expanded `docs/migration/v0.4-to-v0.5.md`).
- **`build.rs` scripts** — automatically configure environment variables to suppress macro warnings in tests/examples
  - `tokitai-macros/build.rs`: quiet mode for the test environment
  - `examples/build.rs`: quiet mode for the example environment

### Changed

- **Design philosophy clarification** — `tokitai` is an **in-process** tool-call library. The `#[tool]` macro generates type-safe `__call_*` wrappers and the `call_tool` dispatcher runs them in your Rust process — zero network hops, zero IPC round-trips after `serde_json::Value` parsing. `tokitai-mcp-server` (and the `mcp` / `http-server` features on `tokitai`) are **optional out-of-process wrappers** that expose the same tool set over HTTP / stdio / SSE; they are not the core. See the `Design philosophy` callout in `README.md` for the canonical wording.
- **Macro warning control logic refactored** — `tokitai-macros/src/tool/mod.rs::should_show_warnings()`
  - Preferentially check the `TOKITAI_SHOW_WARNINGS` environment variable
  - Then check the `TOKITAI_QUIET` environment variable
  - Show warnings by default
- **Version synchronisation** — all workspace crates unified from `0.4.0` to `0.5.0`
- **Error message localisation** — Wrapper-generated `__call_*` strings, the dispatcher fallback, and the `tokitai_core::ToolError` constructors now all use English. Previously the wrappers hard-coded Chinese strings, which caused a single user run to see both languages.

### Performance

- **50% performance improvement** — via systematic `#[inline]` optimisations (X1–X8 series, detailed benchmarks in `docs/performance.md` and `docs/internal/schema-generation-optimization.md`)
- Optimised hot paths: `SchemaGenConfig` Builder methods (17 functions)
- Optimised extraction functions: `extract_param_info`, `extract_doc_comments`
- Optimised code generation: `generate_schema_json_*` family of functions

### Fixed

The eight defects previously documented in `BUGS_FOUND.md` (repurposed as a
regression-test report for this release) are all fixed. The full mapping from
defect to locking test is in `BUGS_FOUND.md`; the short list is:

- **Mixed async + sync methods** in a single `#[tool]` impl no longer produce duplicate `__call_<name>` definitions.
- **Per-parameter `#[tool(default_* = "...")]`** (method-level form) and **`#[tool(validate_<param> = "...")]`** are wired through to runtime behaviour — they used to be accepted by the parser but ignored.
- **Schema/default consistency** — a parameter that has a `default_*` value is no longer listed in the schema's `required` array, and calls that omit it now succeed using the default.
- **Alias description format** — alias descriptions no longer carry the legacy Chinese-language prefix; they match the primary tool's description.
- **Single-language error surface** — wrapper-generated "missing required parameter" / "parameter type mismatch" / "unknown tool" messages are all in English, matching the rest of the runtime.

Additional fix-ups from the W1–W4 round:

- **W1 (P0): Workspace profile configuration fix** — move `[profile.release]` from `tokitai/Cargo.toml` to the workspace root
  - Eliminate the `profiles for the non root package will be ignored` warning
- **W2 (P1): Documentation version number unification** — update all `0.3.3` references to `0.4.0` in documentation
  - `docs/USAGE.md`, `PROMOTION.md`
- **W3 (P2): Macro warning suppression improvement** — use the `TOKITAI_QUIET` environment variable to control macro warning output
  - Added `build.rs` to automatically set the environment variable (test/example environment)
  - Fixed misuse of `cfg!(test)` in procedural macros
- **W4 (P0): Clippy warning cleanup** — fix 12 `default()` call warnings
  - `tokitai-mcp-server/tests/integration_test.rs`: use a unit struct directly for initialisation
- **W4 (P1): Test warning output suppression** — use `cfg!(test)` to suppress macro warning output in the test environment
  - Parameter default value warning (Option type with no default/example)
  - Validation/transformation expression parse failure warning
  - context=async mismatch warning

## [0.4.0] - 2026-03-10

### Breaking Changes

- **API simplification: `TOOL_DEFINITIONS` constant → `tool_definitions()` method**
  - Before: `pub const TOOL_DEFINITIONS: &'static [ToolDefinition] = &[...];`
  - After: `pub fn tool_definitions() -> &'static [ToolDefinition] { ... }`
  - Reason: more flexible runtime tool definition generation, supporting dynamic tool registration
  - Migration: replace all `TOOL_DEFINITIONS` references with `tool_definitions()`

- **Parameter attribute syntax fix**
  - Before: `#[tool_attr(example_format = "...")]` could be placed directly on a parameter
  - After: `#[tool(example_format = "...")]` must be placed at the method level
  - Reason: parameter-level attributes (such as `xxx_param`) must be declared at the method level and handled internally by the macro
  - Migration:
    ```rust
    // OLD syntax (compile error)
    pub fn format_date(&self, date: String, #[tool_attr(example_format = "%Y/%m/%d")] format: String) {}

    // NEW syntax
    #[tool(example_format = "%Y/%m/%d")]
    pub fn format_date(&self, date: String, format: String) {}
    ```

### Fixed

- **P0: Example code compile error** - `examples/mcp_server_demo.rs` parameter attribute conflict
- **P1: Documentation references to old API** - cleaned up 36+ references from `TOOL_DEFINITIONS` → `tool_definitions()`
  - concentrated in `docs/` root and `tokitai/docs/` subdirectory
- **P2: Documentation block syntax error** - macro examples used ` ```text` instead of ` ```rust,ignore`

### Added

- **examples/Cargo.toml complete dependencies** - added HTTP server dependencies such as `axum`, `tower-http`, `tracing-subscriber`
- **P11 review fix report** - `P11_REVIEW_FIX_REPORT_ROUND2.md` completely documents all fixes

### Changed

- **Documentation block conventions** - all macro-generated code examples use ` ```text` syntax highlighting
- **Version synchronisation** - all workspace crate versions unified to `0.4.0`

### Verification

| Check item | Result |
|------------|--------|
| Tests | 85/85 passed |
| Clippy | no warnings |
| Documentation generation | no warnings |
| Compilation | no errors |

---

## [0.3.4] - 2026-03-09

### P11+ Code Review Fixes

#### P0 - Critical Improvements
- **Removed all Chinese compiler warnings** - Complete internationalization
  - Changed `WARNING: method \`add\` is marked deprecated, but no replaced_by is specified` → `[tokitai] warning: method \`add\` is marked deprecated without replaced_by`
  - Changed `WARNING: failed to parse validation expression` → `[tokitai] warning: failed to parse validation expression`
  - Changed `WARNING: failed to parse transform expression` → `[tokitai] warning: failed to parse transform expression`
  - Changed `WARNING: method \`new_method\` is marked context = "async" but is not an async method` → `[tokitai] warning: method \`new_method\` is marked context = "async" but is not an async method`
  - All repair suggestions now in English
- **Refactored `SchemaGenConfig` to public Builder pattern**
  - Before: Private builder methods (internal use only)
  - After: **Public API** - all builder methods now `pub` for external use
  - Added `build()` method for explicit chain termination
  - Example: `SchemaGenConfig::new(params).deprecated(true).tags(&tags).build()`
- **Upgraded global cache from `Mutex<Option>` to `LazyLock<Mutex>`**
  - More idiomatic Rust 1.80+ pattern
  - Eliminates double-wrapping overhead

#### P1 - Quality Improvements
- **Fixed Markdown support documentation** - Avoided over-promising
  - Clarified: "preserves raw text format" instead of "supports Markdown format"
  - Added note: "this function only preserves raw text; it does not perform Markdown parsing (e.g. conversion to HTML)"
  - Recommended external libraries (e.g., `pulldown-cmark`) for full Markdown parsing
- **Added LazyLock deadlock detection warnings**
  - Added `# Safety Note` to `GLOBAL_CONFIG_REGISTRY` documentation
  - Added initialization order warning to `__get_tool_definitions()` documentation
  - Documented current safe initialization sequence
- **Fixed Clippy warnings** (`is_some_and` instead of `map_or`)

#### P2 - Documentation
- **Added comprehensive Cargo Doc example** (`docs/CARGO_DOC_EXAMPLE.md`)
  - How to generate beautiful API documentation
  - Best practices for doc comments with Markdown
  - Custom styling and logo integration
- **Created LangChain Migration Guide** (`docs/LANGCHAIN_MIGRATION.md`)
  - Side-by-side Python vs Rust comparison
  - Complete feature mapping table
  - Performance benchmarks (10-50x faster)
  - FAQ for common migration questions

### Fixed
- Removed all Clippy warnings (25+ warnings → 0)
- Fixed `too_many_arguments` warning by introducing `SchemaGenConfig` struct for JSON Schema generation
- Fixed `dead_code` warning for `build()` method with `#[allow(dead_code)]` attribute
- Fixed unused variable warnings in tests using `cargo fix`

### Changed
- Refactored `generate_schema_json_with_deprecated_and_tags` function signature:
  - Before: 15 individual parameters
  - After: Single `&SchemaGenConfig` parameter for better maintainability
- Simplified code generation logic by removing redundant `#[cfg(feature = "serde")]` conditions

### Improved
- Code quality: All Clippy checks now pass with zero warnings
- Test coverage: 68/68 tests passing (100% pass rate)
- Maintainability: Schema generation configuration now uses structured approach

## [0.3.3] - 2026-03-08

### Note
- Version 0.3.3 was skipped to align version numbers across workspace crates

## [0.3.2] - 2026-03-06

### Added
- `HashCalculator` tool with SHA256 hashing support
  - `sha256()` method for computing string hashes
  - `sha256_file()` method for computing file hashes
- New example demonstrating AI calling SHA256 computation tool
- Dependencies: `sha2` and `hex` crates for cryptographic hashing

### Removed
- HeWeather real API integration due to API host authentication issues
- Weather preloading and caching logic
- `WEATHER_API_KEY` and `WEATHER_USE_REAL_API` environment variables
- `reqwest` blocking feature (now only requires `json` feature)

### Changed
- `WeatherService` now uses mock data for all cities
- Simplified `ollama_integration.rs` example to focus on core functionality

## [0.3.1] - 2026-03-05

### Added
- `Display` trait implementation for `ToolDefinition` for easier debugging
- Comprehensive `starter_project` example template with complete project structure
- `SKILL_TEMPLATE.md` documentation with ready-to-use templates
- Enhanced error messages for generic method violations with actionable suggestions
- Documentation warnings for `call_tool_sync` blocking behavior

### Fixed
- User custom error messages now preserved instead of being replaced with generic "method execution failed"
- Removed unnecessary `#[allow(dead_code)]` attributes in example code
- Doc comment formatting in starter project examples

### Changed
- README.md now includes "5-minute quick start" guide
- README.md now mentions `#[tool(skip)]` feature in features section
- Improved panic error message when calling async methods without runtime
- Starter project now properly organized with modular tool definitions

## [0.3.0] - 2026-03-04

### Added
- `#[tool(skip)]` attribute to exclude public methods from tool registration
- Generic method detection with helpful compile-time error messages
- `call_tool_sync()` now safely handles async methods by checking for tokio runtime
- `log` crate dependency for proper logging (replaces `eprintln!`)
- UI test for `#[tool(skip)]` functionality

### Changed
- **Breaking**: `ToolError::message` changed from `&'static str` to `String` (fixes memory leak)
- **Breaking**: All crate versions bumped to 0.3.0
- Improved `MethodToolAttrs` parsing to support both `#[tool(skip)]` and `#[tool(name = "...", desc = "...")]`
- `get_json_type()` now correctly handles nested `Option<T>` types
- Generated wrapper functions include `#[allow(clippy::all)]` to reduce noise
- MCP schema parsing now logs warnings instead of silently failing
- Example code now follows best practices (Default implementations, etc.)

### Fixed
- Memory leak in `ToolError::message` (removed `Box::leak`)
- `call_tool_sync()` panic when calling async methods without tokio runtime (now returns error)
- Clippy `collapsible_match` warnings in macro code
- Clippy warnings in example code (`new_without_default`, `redundant_closure`, `dead_code`)
- `eprintln!` in library code (replaced with `log::warn!`)

### Removed
- `Box::leak` usage for string allocation (memory-safe implementation)

## [0.2.0] - 2026-03-05

### Changed
- **Breaking**: Renamed macros from `#[ai_skill]`/`#[ai_tool]` to single `#[tool]` macro
- **Breaking**: Simplified API - only one attribute needed
- **Breaking**: Removed runtime dependencies (tokio, reqwest, uuid) from core
- Refactored into three crates: `tokitai-core`, `tokitai-macros`, `tokitai`
- Updated repository to https://github.com/silverenternal/tokitai

### Removed
- **Breaking**: Removed `AnthropicAdapter` and `McpServer` - users can implement their own AI adapters
- Removed business-specific error types for better generality

### Added
- Zero-dependency core types with `&'static str` for compile-time tool definitions
- trybuild UI tests for macro validation
- Optional `mcp` feature for MCP protocol support

## [0.1.0] - 2026-03-05

### Added
- Initial release with `#[ai_skill]` and `#[ai_tool]` macros
- Support for Anthropic (Claude) API adapter
- MCP (Model Context Protocol) server implementation
- Skill registry for managing multiple AI skills
