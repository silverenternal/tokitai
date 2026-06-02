# Deprecated examples

These examples were moved here from the parent `examples/` directory
because they depend on proc-macro attributes that are not part of the
v0.5.0 release: `#[wrap]`, `#[delegate]`, `#[retry]`, `#[rate_limit]`,
`#[circuit_breaker]`, and `#[openapi]`. The corresponding macro
implementations live under `tokitai-macros/src/tool/{wrap,delegate,
resilience,wrap_openapi}/` but are not yet exposed through
`tokitai` / `tokitai_macros`.

The reference documentation that still mentions them
(`docs/wrap-architecture.md`, `docs/reference/{wrap,delegate,retry,
rate-limit,circuit-breaker}.md`) describes the planned public API.

| File | Missing attribute(s) |
|---|---|
| `wrap_native.rs` | `#[wrap]`, `ToolProvider::tool_count` on the wrapped struct |
| `delegate_method.rs` | `#[delegate]`, `__TOOL_DEF_*` / `__call_*` generation |
| `resilient_tool.rs` | `#[retry]`, `#[rate_limit]`, `#[circuit_breaker]`, and a usable `tokitai_macros` re-export surface |
| `wrap_openapi.rs` | `#[openapi]`, `#[openapi_op]`, and a working `openai_chat.json` spec fixture |

The plain `.rs` source is preserved as a design sketch; nothing here
will compile against the current proc-macro crate.
