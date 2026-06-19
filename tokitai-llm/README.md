# tokitai-llm

T-034: command-line harness that drives a `#[tool]`-annotated Rust
provider against an LLM (OpenAI / Anthropic / Ollama), exercises the
tool-calling loop, and persists responses in a blake3-keyed cache so
repeated runs are free.

## Subcommands

| Subcommand | Purpose |
|------------|---------|
| `verify`   | Lint a `ToolDefinition` slice against a JSON-Schema (gated on `schema-verify`). |
| `infer`    | Run a single prompt and print the model's reply, dispatching tool calls in-process. |
| `examples` | Emit JSON-Schema envelopes (`openai-function`, `anthropic-tool`, `mcp-tool`) for every tool in a provider. |

## Quick start

```bash
# Build
cargo build -p tokitai-llm --release

# Run a single prompt against OpenAI
TOKITAI_LLM_PROVIDER=openai \
TOKITAI_LLM_MODEL=gpt-4o \
TOKITAI_LLM_API_KEY=$OPENAI_API_KEY \
tokitai-llm infer -p "What is 2+2?"

# Run against a local Ollama
TOKITAI_LLM_PROVIDER=ollama \
TOKITAI_LLM_MODEL=llama3.1 \
tokitai-llm infer -p "What is 2+2?"

# Lint a JSON-Schema
tokitai-llm verify --schema '{"type":"object","properties":{"x":{"type":"string"}}}'

# Emit envelopes (JSON Lines)
tokitai-llm examples
```

## Design

- **Provider trait** lives in `provider::Provider`. OpenAI, Anthropic,
  and Ollama are concrete impls; the `infer` loop is
  provider-agnostic.
- **Cache** is keyed by a blake3 hash of `(model, system, messages,
  tools)` so a cache hit is bit-identical to the live response.
  Default backend is in-memory; pass `--features sqlite-cache` for
  a persistent SQLite backend.
- **Cross-crate version assertion** (T-024) is implemented via
  `build.rs`, which writes the resolved `tokitai-core` version to
  `OUT_DIR/tokitai_manifest.rs` and re-includes it in `lib.rs`.

## Status

v0.1.0 — infrastructure only. The v0.2 macro-side hooks (T-034
macro side) will plumb a `--provider-crate <path>` arg that points
at a `cdylib` exposing the `#[tool]`-generated `ToolProvider` slice.
