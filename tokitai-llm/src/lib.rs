//! # tokitai-llm
//!
//! T-034: command-line harness that drives a `#[tool]`-annotated Rust
//! provider against an LLM (OpenAI / Anthropic / Ollama), exercises
//! the tool-calling loop, and persists responses in a blake3-keyed
//! cache so repeated runs are free.
//!
//! ## Subcommands
//!
//! - `verify`  - lint a `ToolDefinition` slice against a JSON-Schema
//!   (gated on the `schema-verify` feature).
//! - `infer`   - run a single prompt and print the model's reply,
//!   optionally streaming the tool-call dispatch path.
//! - `examples` - emit JSON-Schema envelopes (`openai-function`,
//!   `anthropic-tool`, `mcp-tool`) for every tool in
//!   a provider; useful for shell-piping into curl.
//!
//! ## Design
//!
//! Provider I/O lives behind a `provider::Provider` trait so swapping
//! between OpenAI, Anthropic, and Ollama is a one-line change. The
//! cache is keyed by a blake3 hash of `(model, system_prompt, messages,
//! tool_envelopes)` so a cache hit is bit-identical to the live
//! response (no parse-time ambiguity).
//!
//! See `docs/MCP_ARCHITECTURE.md` for the parallel that exists in
//! the MCP server, and `docs/AI_INTEGRATION.md` for the broader
//! tool-calling contract.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::needless_return)]

// T-024: the build script writes the resolved `tokitai-core` version
// into OUT_DIR. We re-include it here as a `pub const` so callers
// (and the runtime `infer` path) can read it without re-parsing
// Cargo.lock.
include!(concat!(env!("OUT_DIR"), "/tokitai_manifest.rs"));

pub mod cache;
pub mod cli;
pub mod examples;
pub mod infer;
pub mod infer_capabilities;
pub mod provider;
pub mod verify;

// T-047: re-export the tool-result cache so callers wiring it into
// their own dispatch path do not have to chase the module path.
pub use cache::ToolCache;

// T-050: re-export the provider-middleware types so callers can
// `use tokitai_llm::{ProviderMiddleware, RetryDecision}` without
// having to know the `provider::` module path.
pub use provider::{ProviderMiddleware, RetryDecision, MAX_RETRY_ATTEMPTS};

/// Returned by every subcommand on the happy path. The `anyhow::Error`
/// is the only public error type so the CLI can format a one-line
/// diagnostic with a backtrace when `RUST_BACKTRACE=1` is set.
pub type Result<T> = std::result::Result<T, anyhow::Error>;
