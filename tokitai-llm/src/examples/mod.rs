//! T-034: `tokitai-llm examples` subcommand.
//!
//! Emits JSON-Schema envelopes for every tool in a provider. The
//! output is JSON Lines — one envelope per line — so it can be
//! piped into `jq` or `curl`.
//!
//! The default format (`--format` omitted) emits all three
//! envelopes (`openai-function`, `anthropic-tool`, `mcp-tool`)
//! for every tool, tagged with a `_format` field. Picking a
//! single format emits the raw envelope (no `_format` tag).
//!
//! # Example
//!
//! ```bash
//! $ tokitai-llm examples --format openai-function | jq -c
//! {"type":"function","function":{"name":"add","description":"Add two numbers","parameters":{"type":"object",...}}}
//! ```

use crate::cli::{EnvelopeFormat, ExamplesArgs};
use crate::provider::envelope_for;
use crate::Result;
use tokitai_core::ToolDefinition;

/// Run `tokitai-llm examples` with the given args.
pub async fn run(args: ExamplesArgs) -> Result<()> {
    // T-034: in v0.1 the provider slice is supplied by the
    // embedding host. The CLI stub emits zero envelopes when
    // nothing is wired in, so the binary is well-formed.
    let tools: Vec<ToolDefinition> = Vec::new();

    let formats: &[EnvelopeFormat] = match args.format {
        Some(f) => &[f],
        None => &[
            EnvelopeFormat::OpenaiFunction,
            EnvelopeFormat::AnthropicTool,
            EnvelopeFormat::McpTool,
        ],
    };

    for tool in &tools {
        if let Some(needle) = &args.name_contains {
            if !tool.name.contains(needle.as_str()) {
                continue;
            }
        }
        for fmt in formats {
            let env = envelope_for(tool, *fmt);
            println!("{env}");
        }
    }
    Ok(())
}
