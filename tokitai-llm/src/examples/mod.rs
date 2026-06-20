//! T-034 / T-037: `tokitai-llm examples` subcommand.
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
//! T-037 adds LLM-based example generation: when `--provider`
//! and `--schema` are set, the subcommand calls the LLM to
//! generate one realistic example call per tool and attaches
//! the result as `baked_examples` on the emitted envelope.
//!
//! # Example
//!
//! ```bash
//! $ tokitai-llm examples --format openai-function | jq -c
//! {"type":"function","function":{"name":"add","description":"Add two numbers","parameters":{"type":"object",...}}}
//! ```

use crate::cache::InMemoryCache;
use crate::cli::{EnvelopeFormat, ExamplesArgs};
use crate::infer::{build_provider, complete_with_cache};
use crate::provider::{envelope_for, InferenceRequest};
use crate::Result;
use tokitai_core::ToolDefinition;

/// Run `tokitai-llm examples` with the given args.
pub async fn run(args: ExamplesArgs) -> Result<()> {
    let tools: Vec<ToolDefinition> = match &args.schema {
        Some(text) => serde_json::from_str(text).map_err(|e| {
            anyhow::anyhow!("examples: --schema is not a valid ToolDefinition array: {e}")
        })?,
        None => {
            // v0.1 stub: no provider slice wired in → zero envelopes.
            Vec::new()
        }
    };

    // T-037: generate LLM-based baked_examples for each tool.
    let enriched_tools = if let Some(pa) = &args.provider {
        if let (Some(_kind), Some(_model)) = (&pa.provider, &pa.model) {
            let provider = build_provider(pa)?;
            let cache = InMemoryCache::new();
            // `build_provider` returned `Ok`, so `model` is set.
            let model = pa
                .model
                .as_deref()
                .expect("model validated by build_provider");
            match llm_examples(&tools, &provider, &cache, model).await {
                Ok(examples_map) => apply_examples(tools, &examples_map),
                Err(e) => {
                    eprintln!("warning: LLM examples generation failed: {e}");
                    tools
                }
            }
        } else {
            tools
        }
    } else {
        tools
    };

    let formats: &[EnvelopeFormat] = match args.format {
        Some(f) => &[f],
        None => &[
            EnvelopeFormat::OpenaiFunction,
            EnvelopeFormat::AnthropicTool,
            EnvelopeFormat::McpTool,
        ],
    };

    for tool in &enriched_tools {
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

/// Call the LLM to generate one realistic example call per tool.
///
/// Returns a map of `tool_name -> example_value` where `example_value`
/// is a JSON object representing the `input` fields of a realistic call.
async fn llm_examples(
    tools: &[ToolDefinition],
    provider: &dyn crate::provider::Provider,
    cache: &InMemoryCache,
    model: &str,
) -> std::result::Result<Vec<(String, serde_json::Value)>, anyhow::Error> {
    let mut results = Vec::new();

    for t in tools {
        let schema_pretty = match t.input_schema_value() {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| t.input_schema.clone()),
            Err(_) => t.input_schema.clone(),
        };

        let system = "\
You are a tool-example generator. Given a tool name, description, and JSON-Schema, \
generate ONE realistic example of how to call this tool.

The example should:
- Use realistic values that match the schema types
- Demonstrate proper parameter formatting
- Be concise but meaningful

Reply with ONLY valid JSON in this exact format:
{\"input\":{...}}

Where `input` is an object where each key is a parameter name from the schema \
and each value is a realistic example value matching the parameter's type. \
Do not add markdown fences or commentary.";

        let user_message = format!(
            "Tool name: {}\nDescription: {}\nSchema:\n{}",
            t.name, t.description, schema_pretty
        );

        // T-043: build an `InferenceRequest` instead of passing
        // (system, messages, tools) as separate arguments.
        let mut req = InferenceRequest::new(user_message, Vec::new());
        req.system = Some(system.to_string());
        req.tool_choice = crate::provider::ToolChoice::None;

        let response = match complete_with_cache(provider, cache, false, model, &req).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "warning: LLM example generation failed for `{}`: {e}",
                    t.name
                );
                continue;
            }
        };

        let content = response.content.trim();
        let content = content
            .strip_prefix("```json")
            .or_else(|| content.strip_prefix("```"))
            .unwrap_or(content)
            .strip_suffix("```")
            .unwrap_or(content)
            .trim();

        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(input) = parsed.get("input") {
                results.push((t.name.clone(), input.clone()));
            }
        }
    }

    Ok(results)
}

/// Apply LLM-generated examples to the tool definitions.
///
/// Produces a new `Vec<ToolDefinition>` with `baked_examples` set on
/// each tool that has a corresponding example in the map.
fn apply_examples(
    tools: Vec<ToolDefinition>,
    examples: &[(String, serde_json::Value)],
) -> Vec<ToolDefinition> {
    let mut enriched = Vec::with_capacity(tools.len());

    for mut t in tools {
        if let Some((_, example)) = examples.iter().find(|(name, _)| *name == t.name) {
            // baked_examples is `Option<serde_json::Value>` on ToolDefinition.
            // The shape is `[{"input": {...}}]` — an array of example objects.
            let examples_array = serde_json::Value::Array(vec![serde_json::json!({
                "input": example
            })]);
            t = t.with_baked_examples(examples_array);
        }
        enriched.push(t);
    }

    enriched
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn apply_examples_adds_baked_examples() {
        let t = ToolDefinition::new("add", "Add two numbers", r#"{"type":"object"}"#);
        let examples = vec![("add".to_string(), json!({"a": 1, "b": 2}))];

        let enriched = apply_examples(vec![t], &examples);
        assert_eq!(enriched.len(), 1);
        // baked_examples should be Some(...) after enrichment.
        assert!(enriched[0].baked_examples.is_some());
    }

    #[test]
    fn apply_examples_skips_missing_tool() {
        let t = ToolDefinition::new("sub", "Subtract", r#"{"type":"object"}"#);
        let examples = vec![("add".to_string(), json!({"a": 1, "b": 2}))];

        let enriched = apply_examples(vec![t], &examples);
        assert_eq!(enriched.len(), 1);
        assert!(enriched[0].baked_examples.is_none());
    }

    #[test]
    fn example_json_parsing_works() {
        let json = r#"{"input":{"city":"Tokyo","population":14000000}}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(parsed["input"]["city"], "Tokyo");
    }

    #[test]
    fn apply_examples_multiple_tools() {
        let tools = vec![
            ToolDefinition::new("add", "Add two numbers", r#"{"type":"object"}"#),
            ToolDefinition::new("sub", "Subtract", r#"{"type":"object"}"#),
            ToolDefinition::new("mul", "Multiply", r#"{"type":"object"}"#),
        ];
        // Only "add" and "mul" get matched; "sub" is unmatched.
        let examples = vec![
            ("add".to_string(), json!({"a": 1, "b": 2})),
            ("mul".to_string(), json!({"a": 3, "b": 4})),
        ];

        let enriched = apply_examples(tools, &examples);
        assert_eq!(enriched.len(), 3);
        assert!(enriched[0].baked_examples.is_some());
        assert!(
            enriched[1].baked_examples.is_none(),
            "sub should be skipped"
        );
        assert!(enriched[2].baked_examples.is_some());
    }

    #[test]
    fn apply_examples_baked_shape() {
        let t = ToolDefinition::new("add", "Add two numbers", r#"{"type":"object"}"#);
        let examples = vec![("add".to_string(), json!({"a": 1, "b": 2}))];

        let enriched = apply_examples(vec![t], &examples);
        let baked_str = enriched[0]
            .baked_examples
            .as_ref()
            .expect("baked_examples populated");
        // `baked_examples` is stored as a JSON Value.
        // It should be an array with at least one entry.
        let baked: &serde_json::Value = baked_str;
        // The shape is the canonical `[{...}]` array with a single
        // `{"input": {...}}` object inside it.
        assert!(baked.is_array(), "baked_examples must be a JSON array");
        let arr = baked.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(arr[0].get("input").is_some());
        assert_eq!(arr[0]["input"]["a"], 1);
        assert_eq!(arr[0]["input"]["b"], 2);
    }

    #[test]
    fn apply_examples_no_matches_keeps_tools_unmodified() {
        let t = ToolDefinition::new("add", "Add two numbers", r#"{"type":"object"}"#);
        let examples: Vec<(String, serde_json::Value)> = vec![];

        let enriched = apply_examples(vec![t], &examples);
        assert_eq!(enriched.len(), 1);
        assert!(enriched[0].baked_examples.is_none());
    }
}
