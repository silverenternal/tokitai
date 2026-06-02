# ADR-0005: `#[wrap]` re-uses the `#[tool]` codegen pipeline

- **Status:** Accepted
- **Date:** 2026-06-02
- **Authors:** Tokitai maintainers

## Context

Tokitai v0.4.0 introduced five "wrap" features — `#[wrap]`,
`#[openapi]`, `#[openapi_op]`, `#[delegate]`, plus the three
resilience decorators — that all generate the same downstream
artifacts: `__TOOL_DEF_<NAME>`, `__call_<NAME>`, `__call_<NAME>_sync`,
`call_tool`, the `ToolProvider` impl, and the `ToolCaller` impl.

The wrap features are **not** orthogonal to `#[tool]`. They are
syntax-sugar front-ends that differ from `#[tool]` in two ways:

- `#[wrap]` is `#[tool]` with a curated method list and a
  constructor (`new(client: T) -> Self`).
- `#[openapi]` is `#[tool]` with spec-derived method metadata and a
  `phf::Map` lookup.
- `#[delegate]` is `#[tool]` with the body synthesised by a
  `forward to <expr>` rule.

The natural temptation is to write a parallel codegen pipeline for
each — `wrap::codegen`, `openapi::codegen`, `delegate::codegen` —
that re-implements the parameter parsing, the validation, the
result serialisation, and the dispatcher. Each pipeline would
naturally be ~3000 lines of code, with small but real differences
in how each macro handles a corner case (e.g. an `Option<Option<T>>`
parameter).

## Decision

All wrap features share the **same** downstream codegen pipeline as
`#[tool]`. The proc-macro entry point for each feature parses the
attribute, builds a `Vec<ToolMethodInfo>` (the same struct `#[tool]`
uses internally), and then hands off to the `tool::codegen::*`
helpers.

The wrap features differ from `#[tool]` only in:

1. **How the `Vec<ToolMethodInfo>` is built.** `#[wrap]` takes a
   user-supplied subset; `#[openapi]` walks the parsed spec;
   `#[delegate]` parses a single forward signature.
2. **What extra items are emitted.** `#[wrap]` emits a `new()`
   constructor; `#[openapi]` emits a `phf::Map` and the raw spec
   text; `#[delegate]` emits nothing extra.

The shared pipeline is
[`tokitai-macros/src/tool/codegen/`](../../tokitai-macros/src/tool/codegen/),
specifically `wrappers.rs` (846 lines) and the dispatcher emitter.

## Consequences

**Easier:**

- A bug fix in `#[tool]`'s codegen immediately benefits `#[wrap]`,
  `#[openapi]`, and `#[delegate]`. One fix, four macros updated.
- New validation rules (e.g. a new `min_length` constraint) are
  available across all five macros at once, because the validation
  code is shared.
- The per-macro file (`wrap/codegen.rs`, `wrap_openapi/codegen.rs`,
  `delegate/codegen.rs`) is small — each one is essentially
  "build a `ToolMethodInfo` list, then call into the shared
  pipeline". The bulk of the codegen lives in one place.
- Documentation for `__TOOL_DEF_*` and `__call_*` only has to
  be written once. Users see identical artifacts regardless of
  which front-end they used to produce them.

**Harder:**

- The wrap features inherit every limitation of the `#[tool]`
  pipeline. Today this means: generic methods are not supported
  (the codegen cannot synthesise a monomorphised wrapper for a
  generic), `Self`-typed parameters are not supported, and
  lifetimes in parameter positions are passed through verbatim.
  Any of these is a feature gap for one of the wrap macros.
- Adding a new wrap feature requires understanding the
  `ToolMethodInfo` shape, not just the user-facing attribute
  syntax. The learning curve for a new contributor is higher than
  it would be with a "one codegen per macro" design.
- The shared codegen has grown to ~1500 lines. It is no longer
  possible to read the entire pipeline end-to-end in one sitting.
  A new contributor needs to start at the `tool_method_info`
  extraction, then the wrappers, then the dispatcher.

## Alternatives considered

- **Separate `WrapCodegen`, `OpenApiCodegen`, `DelegateCodegen`** —
  rejected. Three copies of the same 3000 lines of codegen is
  ~9000 lines that must be kept in sync. The duplication would
  silently drift the moment one of them got a bug fix the others
  did not. A user reporting "the `min` constraint does not work
  on my `#[wrap]` method" would be a release-blocking surprise
  for the v0.5.x series.
- **Macro that generates a `#[tool]` impl from a `#[wrap]`** —
  rejected. `#[wrap]` would expand to a fresh `#[tool]` impl,
  which is a layer of indirection that would only be visible in
  compiler errors. The user would see error messages that point
  at the generated `#[tool]` impl, not at their `#[wrap]`
  attribute, and the errors would be harder to act on. The direct
  "share the codegen" approach keeps error spans local.
