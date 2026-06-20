//! T-034: `clap`-derived CLI surface for the `tokitai-llm` binary.
//!
//! Subcommand tree:
//! - `verify`  - lint tool schemas
//! - `infer`   - run a single prompt
//! - `examples` - emit JSON-Schema envelopes for every tool
//!
//! The CLI is intentionally model-agnostic: the `--provider` flag
//! picks the `provider::Provider` implementation, and the rest of
//! the surface stays the same.

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Top-level CLI arguments. Parsed once in `main`; the resolved
/// `Command` variant is forwarded to the matching subcommand handler.
#[derive(Debug, Parser)]
#[command(
    name = "tokitai-llm",
    version,
    about = "Tokitai LLM CLI - tool-calling harness for #[tool] providers",
    long_about = "Drive a #[tool]-annotated Rust provider against an LLM \
                  (OpenAI / Anthropic / Ollama), dispatch the returned \
                  tool calls in-process, and cache the responses."
)]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The exhaustive list of `tokitai-llm` subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Lint every `#[tool]` definition in a provider against a
    /// JSON-Schema. Fails on the first mismatch.
    #[command(name = "verify")]
    Verify(VerifyArgs),

    /// Run a single prompt and print the model's reply, dispatching
    /// any tool calls through the in-process provider.
    #[command(name = "infer")]
    Infer(InferArgs),

    /// Emit JSON-Schema envelopes (`openai-function`,
    /// `anthropic-tool`, `mcp-tool`) for every tool in a provider.
    /// The output is JSON Lines — one envelope per line.
    #[command(name = "examples")]
    Examples(ExamplesArgs),

    /// Infer capability requirements for tool definitions using an
    /// LLM. Reads tool definitions from --schema and asks the model
    /// to suggest requires = [...] entries per tool.
    #[command(name = "infer-capabilities")]
    InferCapabilities(InferCapabilitiesArgs),
}

/// Which LLM provider to talk to. The enum maps 1:1 onto
/// `provider::Provider` implementations.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ProviderKind {
    /// OpenAI Chat Completions API (`/v1/chat/completions`).
    Openai,
    /// Anthropic Messages API (`/v1/messages`).
    Anthropic,
    /// Ollama native API (`/api/chat`). Useful for local development.
    Ollama,
}

/// Shared provider/credential arguments used by every subcommand.
#[derive(Debug, Args, Clone)]
pub struct ProviderArgs {
    /// Which LLM provider to talk to. Required for `infer`; the
    /// other subcommands emit the envelope format that pairs with
    /// the chosen provider.
    #[arg(long, value_enum, env = "TOKITAI_LLM_PROVIDER")]
    pub provider: Option<ProviderKind>,

    /// Base URL of the provider API. Defaults to the public
    /// endpoint of the chosen provider.
    #[arg(long, env = "TOKITAI_LLM_BASE_URL")]
    pub base_url: Option<String>,

    /// Model name (e.g. `gpt-4o`, `claude-3-5-sonnet-latest`,
    /// `llama3.1`). Required for `infer`.
    #[arg(long, env = "TOKITAI_LLM_MODEL")]
    pub model: Option<String>,

    /// API key. Read from the env var named by `--api-key-env`
    /// (default `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` /
    /// `OLLAMA_API_KEY` depending on `--provider`).
    #[arg(long, env = "TOKITAI_LLM_API_KEY")]
    pub api_key: Option<String>,

    /// Maximum tokens to request. Anthropic requires this field
    /// on every Messages request; OpenAI and Ollama treat it as
    /// an upper bound on completion length. Defaults to the
    /// provider-specific [`crate::provider::anthropic::DEFAULT_MAX_TOKENS`]
    /// for Anthropic and is ignored by OpenAI / Ollama.
    #[arg(long, env = "TOKITAI_LLM_MAX_TOKENS")]
    pub max_tokens: Option<u64>,
}

/// `tokitai-llm verify` arguments.
#[derive(Debug, Args, Clone)]
pub struct VerifyArgs {
    /// Path to a Rust source file that defines a `#[tool]`-annotated
    /// provider. Currently informational — the verifier operates on
    /// the provider slice passed in by an embedding host.
    #[arg(long)]
    pub provider: Option<String>,

    /// Inline JSON-Schema to lint. Mutually exclusive with
    /// `--provider` (the resolver picks whichever is set).
    #[arg(long)]
    pub schema: Option<String>,

    /// When set, the verifier exits 0 even if it found issues
    /// (useful for shell `set -e` workflows that just want a report).
    #[arg(long)]
    pub no_fail: bool,

    /// Provider configuration (URL, key, model). When set, the
    /// verifier also runs an LLM-based description-quality check
    /// and merges the findings with the syntactic pass.
    #[command(flatten)]
    pub provider_args: ProviderArgs,

    /// Path to write the verification report JSON. When set, the
    /// report is written there; otherwise it is printed to stdout
    /// as JSON at the end of the run.
    #[arg(long)]
    pub report_path: Option<String>,
}

/// `tokitai-llm infer` arguments.
#[derive(Debug, Args, Clone)]
pub struct InferArgs {
    /// The user prompt to send to the model.
    #[arg(long, short = 'p')]
    pub prompt: String,

    /// Optional system prompt.
    #[arg(long, short = 's')]
    pub system: Option<String>,

    /// Provider configuration (URL, key, model).
    #[command(flatten)]
    pub provider: ProviderArgs,

    /// Bypass the response cache. Default is to use the cache.
    #[arg(long)]
    pub no_cache: bool,

    /// Disable streaming (default: stream chunks as they arrive).
    #[arg(long)]
    pub no_stream: bool,

    /// Maximum number of tool-call round-trips before giving up.
    /// Defaults to 16 to match the OpenAI / Anthropic recommended
    /// upper bound.
    #[arg(long, default_value_t = 16)]
    pub max_iterations: usize,

    /// TTL in seconds for the tool-result cache. Repeated
    /// `(tool_name, args)` lookups within this window reuse the
    /// cached value instead of re-dispatching to the underlying
    /// tool provider. The cache is keyed on `(tool_name,
    /// serialised arguments)` — not on the full chat envelope —
    /// so a self-consistency run that issues the same tool call
    /// 5 times collapses to a single dispatch.
    #[arg(long, default_value_t = 60, env = "TOKITAI_LLM_TOOL_CACHE_TTL")]
    pub tool_cache_ttl: u64,

    /// Maximum number of entries the tool-result cache holds.
    /// `0` disables the cache entirely (every tool call reaches
    /// the provider). Defaults to 1000, which covers the
    /// self-consistency 5-sample path with room to spare.
    #[arg(long, default_value_t = 1000, env = "TOKITAI_LLM_TOOL_CACHE_SIZE")]
    pub tool_cache_size: usize,
}

/// `tokitai-llm examples` arguments.
#[derive(Debug, Args, Clone)]
pub struct ExamplesArgs {
    /// Which envelope format to emit (`openai-function`,
    /// `anthropic-tool`, `mcp-tool`). Defaults to all three,
    /// one per line (JSON Lines output).
    #[arg(long, value_enum)]
    pub format: Option<EnvelopeFormat>,

    /// Optional filter: only emit envelopes whose tool name
    /// matches this substring.
    #[arg(long)]
    pub name_contains: Option<String>,

    /// Provider configuration (URL, key, model). When set, the
    /// examples subcommand also generates LLM-based example values
    /// for every tool's parameters and attaches them as baked_examples.
    #[command(flatten)]
    pub provider: Option<ProviderArgs>,

    /// Inline tool-definitions JSON array. When set alongside
    /// --provider, the examples subcommand generates baked_examples
    /// using LLM calls instead of emitting empty envelopes.
    #[arg(long)]
    pub schema: Option<String>,
}

/// `tokitai-llm infer-capabilities` arguments.
#[derive(Debug, Args, Clone)]
pub struct InferCapabilitiesArgs {
    /// Provider configuration (URL, key, model).
    #[command(flatten)]
    pub provider: ProviderArgs,

    /// Inline tool-definitions JSON array. Each entry must have
    /// `name`, `description`, and `input_schema` fields matching
    /// the `ToolDefinition` shape.
    #[arg(long)]
    pub schema: Option<String>,

    /// Bypass the LLM response cache.
    #[arg(long, default_value_t = false)]
    pub no_cache: bool,
}

/// JSON-Schema envelope flavours emitted by `tokitai-llm examples`.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum EnvelopeFormat {
    /// OpenAI `function` envelope (`{"type":"function","function":{...}}`).
    OpenaiFunction,
    /// Anthropic `tool` envelope (`{"name":...,"description":...,"input_schema":{...}}`).
    AnthropicTool,
    /// MCP `tool` envelope (`{"name":...,"description":...,"inputSchema":{...}}`).
    McpTool,
}
