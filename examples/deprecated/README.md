# Deprecated examples

The previous `examples/deprecated/{wrap_native,delegate_method,
resilient_tool,wrap_openapi}.rs` files were placeholders for
proc-macro attributes (`#[wrap]`, `#[delegate]`, `#[retry]`,
`#[rate_limit]`, `#[circuit_breaker]`, `#[openapi]` /
`#[openapi_op]`) that are **not yet exposed by `tokitai` /
`tokitai_macros` in 0.5.x**. They were deleted as part of T-007
because they neither compiled against the public API nor pointed
users at a runnable replacement.

The corresponding macro *implementations* still live under
`tokitai-macros/src/tool/{wrap,delegate,resilience,wrap_openapi}/`
and are unit-tested in `tokitai-macros/tests/`, but they are not
registered as `#[proc_macro_attribute]` entry points in
`tokitai-macros/src/lib.rs`. Tracking issues for each attribute
(so users can see the gap is intentional and follow progress) are
listed below.

| Attribute(s) | Status | Tracking issue |
|--------------|--------|----------------|
| `#[wrap]` | Implementation in tree, attribute not exported | tracking-issue: <https://github.com/silverenternal/tokitai/issues/31> |
| `#[delegate]` | Implementation in tree, attribute not exported | tracking-issue: <https://github.com/silverenternal/tokitai/issues/32> |
| `#[retry]` | Implementation in tree, attribute not exported | tracking-issue: <https://github.com/silverenternal/tokitai/issues/33> |
| `#[rate_limit]` | Implementation in tree, attribute not exported | tracking-issue: <https://github.com/silverenternal/tokitai/issues/34> |
| `#[circuit_breaker]` | Implementation in tree, attribute not exported | tracking-issue: <https://github.com/silverenternal/tokitai/issues/35> |
| `#[openapi]` / `#[openapi_op]` | Implementation in tree, attribute not exported | tracking-issue: <https://github.com/silverenternal/tokitai/issues/36> |

In the meantime, see `examples/basic_usage.rs`,
`examples/dev_assistant.rs`, `examples/multi_tool_chat.rs`, and
`examples/database_tool/` for runnable end-to-end patterns that
use the **currently exported** `#[tool]` macro and the
`MultiToolProvider` runtime composition surface (which is what
`#[wrap]` / `#[delegate]` are designed to wrap, eventually).

The reference documentation in
`docs/wrap-architecture.md` and
`docs/reference/{wrap,delegate,retry,rate-limit,circuit-breaker,
openapi}.md` still describes the planned public API; consult the
tracking issues above for the export schedule.
