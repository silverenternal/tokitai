# Tokitai Tutorials

Step-by-step, runnable guides for the Tokitai proc-macro crate.

## Available tutorials

| Tutorial                                            | Audience                                  | Length  |
|-----------------------------------------------------|-------------------------------------------|---------|
| [Getting Started](getting-started.md)               | First read after `cargo add tokitai`      | ~30 min |

The **Getting Started** tutorial is a five-chapter walk-through:

1. **Hello, tool** — your first `#[tool]` method, end-to-end.
2. **Multiple tools + parameter validation** — `tool_min`, `tool_max`, `tool_pattern`, `tool_required`.
3. **Async tools and the runtime-agnostic bridge** — `async fn` and the `AsyncExecutor` trait.
4. **Resilience decorators** — `#[retry]`, `#[rate_limit]`, `#[circuit_breaker]`.
5. **Wrapping a third-party API** — `#[wrap]`, `#[openapi]`, `#[delegate]`.

Each chapter is standalone: copy the code into a fresh `src/main.rs`
and run it. The chapters form a progression from "hello world" to
production-grade.

## How to use these tutorials

If you're new to Tokitai, read the chapters in order. If you're
evaluating a specific feature, jump to the relevant chapter — every
chapter's code is self-contained.

## Related reading

- [`../wrap-architecture.md`](../wrap-architecture.md) — long-form
  reference for `#[wrap]`, `#[openapi]`, `#[delegate]`, and the
  three resilience decorators.
- [`../wrap-cheatsheet.md`](../wrap-cheatsheet.md) — one-page
  reference card.
- [`../quickstart.md`](../quickstart.md) — existing five-minute
  quick start (kept for terseness; this directory is the
  progressive counterpart).
- [`../../examples/`](../../examples/) — runnable example programs
  for every feature.
