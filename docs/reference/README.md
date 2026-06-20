# Tokitai Attribute Reference

**Version**: 0.6.0 | **Crate**: [`tokitai`](https://crates.io/crates/tokitai)

This directory is the per-attribute API reference for Tokitai. Every proc-macro
attribute and macro in `tokitai-macros` gets one page, with a uniform structure
so you can learn one and skim the rest.

If you are new to Tokitai, read [`docs/quickstart.md`](../quickstart.md) and
[`docs/USAGE.md`](../USAGE.md) first; this directory is for the "I know the
concept, what does this attribute actually do" stage.

---

## Block-level attributes

These attributes go on an `impl` block and affect the whole type.

| Attribute / macro | Page | One-liner |
|---|---|---|
| `#[tool]` | [`tool.md`](tool.md) | Mark an `impl` block; every `pub` method becomes an AI tool. |
| `#[tool_type]` | [`tool-type.md`](tool-type.md) | Attach an explicit JSON Schema to a custom struct. |
| `config!` | [`config.md`](config.md) | Runtime override of tool descriptions, tags, and param hints. |
| `#[wrap]` | [`wrap.md`](wrap.md) | Same as `#[tool]` but with a curated `methods = [...]` list and a generated `new(client)` constructor. |
| `#[openapi]` + `#[openapi_op]` | [`openapi.md`](openapi.md) | Drive the wrapper from an OpenAPI 3 spec file. |

## Method-level attributes

These go on a method inside an `impl` block (or in the case of `#[delegate]`,
on a free-standing signature).

| Attribute | Page | One-liner |
|---|---|---|
| `#[delegate]` | [`delegate.md`](delegate.md) | Forward a method to an inner expression without writing a body. |
| `#[retry]` | [`retry.md`](retry.md) | Retry the body on `Err` with backoff + jitter. |
| `#[rate_limit]` | [`rate-limit.md`](rate-limit.md) | Token-bucket throttle on a per-method basis. |
| `#[circuit_breaker]` | [`circuit-breaker.md`](circuit-breaker.md) | 3-state (closed / open / half-open) breaker on consecutive failures. |

## Parameter-level attributes

These go on a parameter inside a `#[tool]` method.

| Attribute(s) | Page | One-liner |
|---|---|---|
| `#[tool_min]`, `#[tool_max]`, `#[tool_pattern]`, `#[tool_required]`, `#[tool_default]`, `#[tool_example]`, `#[tool_validate]`, `#[tool_transform]`, `#[tool_desc]`, `#[tool_alias]`, `#[tool_hidden]`, `#[tool_deprecated]`, `#[tool_min_length]`, `#[tool_max_length]`, `#[tool_min_items]`, `#[tool_max_items]`, `#[tool_multiple_of]` | [`param-attrs.md`](param-attrs.md) | Per-parameter JSON-Schema constraints, defaults, and examples. |

> **Note**: `param_tool` is the bundled form (`#[param_tool(...)]`) of the
> per-parameter attributes; it accepts the same keys in one attribute
> group. See [`param-attrs.md`](param-attrs.md#bundled-form-param_tool).

---

## How to read each page

Every page follows the same nine-section structure:

1. **Syntax** — minimal Rust sketch of where the attribute goes.
2. **Arguments** — a table of every accepted key, its type, default, and
   meaning.
3. **Examples** — three blocks: *Minimal*, *Common usage*, *Edge case*.
4. **Generated code** — what the macro actually expands to. This is
   extracted from the macro source so it is byte-for-byte accurate.
5. **Interactions** — which other attributes compose with this one, which
   traits it implements, which methods it adds.
6. **Errors** — every `compile_error!` message the macro can produce,
   cross-referenced to the error-code index.
7. **See also** — the relevant example file, the wrap-architecture
   doc, and the rustdoc.

If a section is short for a given attribute, it is intentionally short:
the goal is that you can open any one of the ten files and find the
answers in the same place.

---

## Cross-references

- [`docs/wrap-architecture.md`](../wrap-architecture.md) — the
  feature matrix and composition rules for the wrap features
  (`#[wrap]`, `#[openapi]`, `#[delegate]`, the three resilience
  decorators).
- [`docs/wrap-cheatsheet.md`](../wrap-cheatsheet.md) — a one-page
  cheat sheet comparing the same five features.
- [`docs/USAGE.md`](../USAGE.md) — the long-form tutorial-style walk
  through `#[tool]`.
- [`docs/quickstart.md`](../quickstart.md) — the 5-minute tour.
- [`docs/AI_INTEGRATION.md`](../AI_INTEGRATION.md) — the per-provider
  envelope details (`to_openai_function`, `to_anthropic_tool`,
  `to_mcp_tool`).
- [`docs/CROSS_LANGUAGE.md`](../CROSS_LANGUAGE.md) — HTTP/JSON
  protocol and the four client SDKs in `examples/curl/`,
  `examples/py/`, `examples/js/`, `examples/go/`.
- [`docs/API_STABILITY.md`](../API_STABILITY.md) — semver policy for
  the wrap features.
- [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) — overall Tokitai
  architecture.

## Per-example links

The `See also` section of every reference page points to the relevant
example file under `examples/`. The full list:

| Example | Attributes it demonstrates |
|---|---|
| [`examples/basic_usage.rs`](../../examples/basic_usage.rs) | `#[tool]` |
| [`examples/param_attrs.rs`](../../examples/param_attrs.rs) | per-parameter `#[tool(...)]` keys |
| [`examples/validate_transform_alias.rs`](../../examples/validate_transform_alias.rs) | `validate`, `transform`, `alias` |
| [`examples/wrap_demo.rs`](../../examples/wrap_demo.rs) | pattern demo for `#[wrap]` / `#[delegate]` curated surfaces (uses the stable `#[tool]` + `MultiToolProvider`; see [`deprecated/`](../../examples/deprecated/) for the export schedule) |
| [`examples/wrap_openapi.rs`](../../examples/wrap_openapi.rs) | `#[openapi]`, `#[openapi_op]` (tracking-issue: [#36](https://github.com/silverenternal/tokitai/issues/36)) |
| [`examples/mcp_http_server.rs`](../../examples/mcp_http_server.rs) | full `#[tool]` server with the `tokitai` envelope methods |
| [`examples/runtime_agnostic.rs`](../../examples/runtime_agnostic.rs) | runtime-agnostic async (registers an `AsyncExecutor`) |
| [`examples/advanced_types.rs`](../../examples/advanced_types.rs) | `#[tool_type]` and rich struct schemas |
| [`examples/multi_tool_chat.rs`](../../examples/multi_tool_chat.rs) | multiple `#[tool]` impl blocks in one binary |
| [`examples/ollama_integration.rs`](../../examples/ollama_integration.rs) | Ollama integration using `to_anthropic_tool` |
| [`examples/mcp_server_demo.rs`](../../examples/mcp_server_demo.rs) | the in-process MCP demo |
| [`examples/debug_tools.rs`](../../examples/debug_tools.rs) | `#[tool]` with debug logging |
| [`examples/starter_project/`](../../examples/starter_project/) | blank slate you can copy |

> The `#[wrap]` / `#[delegate]` / `#[retry]` / `#[rate_limit]` / `#[circuit_breaker]` / `#[openapi]` attributes are implemented in `tokitai-macros` but are not yet exported in 0.5.x. See [`examples/deprecated/README.md`](../../examples/deprecated/README.md) for the tracking-issue table.
