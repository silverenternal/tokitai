# Architecture Decision Records

This directory contains the Architecture Decision Records (ADRs) for
Tokitai. ADRs document the **why** behind major design decisions: the
constraints we were under, the options we considered, the trade-offs we
accepted, and the consequences we now live with. The **what** lives in
the code; the **why** lives here.

## How to read these

Each ADR follows the same template:

- **Context** — the problem and the constraints
- **Decision** — what we picked
- **Consequences** — what becomes easier, what becomes harder
- **Alternatives considered** — what we rejected, and why

ADRs are dated, attributed, and (in the future) superseded in place.
When a decision is reversed, the original ADR is left in place with its
status set to `Superseded by NNNN` and a new ADR is added. We do not
delete history.

## Index

| ADR                                                           | Title                                                                                              | Status   | Date       |
|---------------------------------------------------------------|----------------------------------------------------------------------------------------------------|----------|------------|
| [ADR-0001](0001-async-executor-type-erasure.md)               | `AsyncExecutor` uses type erasure, not generics                                                     | Accepted | 2026-06-02 |
| [ADR-0002](0002-phf-map-for-openapi-ops.md)                   | OpenAPI operations use `phf::Map`, not `HashMap`                                                   | Accepted | 2026-06-02 |
| [ADR-0003](0003-sync-from-async-via-block-on-dyn.md)          | Sync-from-async bridge uses `block_on_dyn`, not `block_on_async`                                    | Accepted | 2026-06-02 |
| [ADR-0004](0004-circuit-breaker-v1-observe-only.md)           | `#[circuit_breaker]` v1 is observe-only, not fail-fast                                             | Accepted | 2026-06-02 |
| [ADR-0005](0005-wrap-reuses-tool-codegen.md)                  | `#[wrap]` re-uses the `#[tool]` codegen pipeline                                                    | Accepted | 2026-06-02 |
| [ADR-0006](0006-openapi-spec-path-resolution.md)              | Spec path resolution for `#[openapi]` uses `Span::local_file()`                                     | Accepted | 2026-06-02 |

## Status legend

- **Accepted** — the decision is current and in force
- **Superseded by NNNN** — the decision has been replaced; see the
  linked ADR for the new direction
- **Deprecated** — the decision is no longer recommended for new
  code, but is still in effect for existing code

## Conventions

- All ADRs use 4-digit zero-padded numbers (`0001`, `0002`, ...).
- ADRs are append-only; never rewrite an existing ADR.
- New ADRs are added with the next free number.
- "Supersedes" / "Superseded by" cross-references live in the front
  matter.
- Length budget: 50-100 lines per ADR. If a decision needs more than
  that, it is usually two decisions in a trench coat; split it.
