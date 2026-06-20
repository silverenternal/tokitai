//! T-036: `tokitai-llm infer-capabilities` subcommand.
//!
//! Reads tool definitions from `--schema` and asks an LLM to suggest
//! `requires = [...]` entries — capability strings like `"db:read"`,
//! `"fs:write"`, `"net:http"` — that each tool should declare.
//!
//! The capability suggestions are printed as JSON Lines so they can
//! be piped into a build script or manual review step.

use crate::cache::InMemoryCache;
use crate::cli::InferCapabilitiesArgs;
use crate::infer::{build_provider, complete_with_cache};
use crate::provider::InferenceRequest;
use crate::Result;
use serde::Serialize;
use tokitai_core::ToolDefinition;

/// One capability suggestion from the LLM.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct CapabilitySuggestion {
    /// Tool name the capability applies to.
    tool: String,
    /// Suggested `requires = [...]` capability strings.
    capabilities: Vec<String>,
}

/// Run `tokitai-llm infer-capabilities` with the given args.
pub async fn run(args: InferCapabilitiesArgs) -> Result<()> {
    let schema_text = match args.schema {
        Some(s) => s,
        None => {
            anyhow::bail!(
                "infer-capabilities: pass --schema <json> with a JSON array of \
                 tool definitions; got nothing"
            );
        }
    };

    // Need both provider and model for LLM calls.
    let _ = args
        .provider
        .provider
        .ok_or_else(|| anyhow::anyhow!("infer-capabilities: --provider is required"))?;
    let _ = args
        .provider
        .model
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("infer-capabilities: --model is required"))?;

    let tools: Vec<ToolDefinition> = serde_json::from_str(&schema_text).map_err(|e| {
        anyhow::anyhow!("infer-capabilities: --schema is not a valid ToolDefinition array: {e}")
    })?;

    if tools.is_empty() {
        anyhow::bail!("infer-capabilities: --schema contains zero tool definitions");
    }

    let provider = build_provider(&args.provider)?;
    let cache = InMemoryCache::new();
    // `build_provider` returned `Ok` above, so `model` is set.
    let model = args
        .provider
        .model
        .as_deref()
        .expect("model validated by build_provider");

    let suggestions = llm_infer_capabilities(&tools, &provider, &cache, model).await?;

    // JSON Lines output (one suggestion per line). The doc comment
    // at the top of this file promises `jq`-pipeable NDJSON; pretty
    // printing the whole array would break that contract.
    for s in &suggestions {
        let line = serde_json::to_string(s)?;
        println!("{line}");
    }

    Ok(())
}

/// Send tool definitions to the LLM and ask for capability suggestions.
async fn llm_infer_capabilities(
    tools: &[ToolDefinition],
    provider: &dyn crate::provider::Provider,
    cache: &InMemoryCache,
    model: &str,
) -> std::result::Result<Vec<CapabilitySuggestion>, anyhow::Error> {
    let mut tool_parts: Vec<String> = Vec::new();
    for t in tools {
        let schema_pretty = match t.input_schema_value() {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| t.input_schema.clone()),
            Err(_) => t.input_schema.clone(),
        };
        tool_parts.push(format!(
            "Tool: {}\nDescription: {}\nSchema:\n{}",
            t.name, t.description, schema_pretty
        ));
    }
    let tools_text = tool_parts.join("\n---\n");

    let system = "\
You are a capability inference engine. Given tool definitions, suggest what \
capabilities (as short Rust &str tags) each tool should require.

Capabilities are short strings describing what a tool needs access to. Examples:
- \"db:read\" for tools that read from a database
- \"db:write\" for tools that write to a database
- \"fs:read\" for tools that read files
- \"fs:write\" for tools that write files
- \"net:http\" for tools that make HTTP requests
- \"calc:arithmetic\" for basic math tools
- \"search:web\" for web search tools
- \"code:execute\" for code execution tools
- \"auth:user\" for user authentication tools
- \"email:send\" for email sending tools

Reply with ONLY valid JSON in this exact format:
{\"suggestions\":[{\"tool\":\"tool_name\",\"capabilities\":[\"cap1\",\"cap2\"]}]}

If a tool has no specific capability requirement, suggest [\"general\"]. \
Do not add markdown fences or commentary.";

    let user_message = format!("Tool definitions to analyze:\n\n{tools_text}");

    // T-043: build an `InferenceRequest` instead of passing
    // (system, messages, tools) as separate arguments.
    let mut req = InferenceRequest::new(user_message, Vec::new());
    req.system = Some(system.to_string());
    req.tool_choice = crate::provider::ToolChoice::None;

    let response = complete_with_cache(provider, cache, false, model, &req).await?;

    let content = response.content.trim();
    let content = content
        .strip_prefix("```json")
        .or_else(|| content.strip_prefix("```"))
        .unwrap_or(content)
        .strip_suffix("```")
        .unwrap_or(content)
        .trim();

    #[derive(serde::Deserialize)]
    struct LlmResponse {
        suggestions: Vec<CapabilitySuggestion>,
    }

    let parsed: LlmResponse = serde_json::from_str(content)?;
    Ok(parsed.suggestions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_suggestion_serializes() {
        let s = CapabilitySuggestion {
            tool: "add".to_string(),
            capabilities: vec!["calc:arithmetic".to_string()],
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("calc:arithmetic"));
        assert!(json.contains("add"));
    }

    #[test]
    fn empty_capabilities_list_serializes() {
        let list = vec![CapabilitySuggestion {
            tool: "empty_tool".to_string(),
            capabilities: Vec::new(),
        }];
        let json = serde_json::to_string(&list).unwrap();
        assert!(json.contains("empty_tool"));
    }
}
