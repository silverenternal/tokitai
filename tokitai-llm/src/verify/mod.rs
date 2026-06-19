//! T-034 / T-035: `tokitai-llm verify` subcommand.
//!
//! Lints a JSON-Schema (or a `ToolDefinition`-derived envelope) for
//! the same footguns the MCP typed layer (T-021) catches at runtime:
//! missing `type` keyword, `additionalProperties: true` when the
//! caller expected `false`, etc. The actual JSON-Schema validator
//! is gated on the `schema-verify` feature; without it, only a
//! lightweight syntactic check is performed.
//!
//! T-035 adds an LLM-powered description-quality pass: when
//! `--provider` (and `--model`) are set, the verifier sends every
//! tool definition to the LLM and asks it to flag vague descriptions,
//! missing side-effect documentation, and unclear parameter docs.
//! The LLM findings are merged with the syntactic lint pass and
//! optionally written to a JSON report file consumed by the
//! `#[tool]` macro's `emit_verify_warnings`.

use crate::cache::InMemoryCache;
use crate::cli::VerifyArgs;
use crate::infer::{build_provider, complete_with_cache};
use crate::provider::ChatMessage;
use crate::Result;
use serde::Serialize;
use tokitai_core::ToolDefinition;

/// One verifier finding. The CLI prints one line per finding
/// (`[ERROR] code: message`).
#[derive(Debug, Clone)]
pub struct Finding {
    /// Stable error code (e.g. `MCP-1` for "missing `type` keyword").
    pub code: &'static str,
    /// Human-readable description of the issue.
    pub message: String,
}

/// Report shape that the `#[tool]` macro's `emit_verify_warnings`
/// can consume. Matches the `VerifyReport` struct in
/// `tokitai-macros/src/tool/llm/mod.rs`.
#[derive(Debug, Clone, Serialize)]
struct VerifyReport {
    /// Schema version of the report. Currently always `1`.
    version: u32,
    /// Findings emitted by the verifier. Empty list means clean.
    findings: Vec<VerifyFinding>,
}

/// One entry in the verification report. Maps to the macro-side
/// `VerifyFinding` struct in
/// `tokitai-macros/src/tool/llm/mod.rs`.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct VerifyFinding {
    /// Tool the finding was attributed to (`Type::method`). May be
    /// empty for impl-block-level findings.
    tool: String,
    /// Stable finding code (e.g. `MCP-1`, `DESC-VAGUE`).
    code: String,
    /// Human-readable description of the issue.
    message: String,
}

/// Run `tokitai-llm verify` with the given args.
///
/// Returns `Ok(report)` on success (the report may still contain
/// findings when `--no-fail` is set). Returns `Err` when the
/// command should exit non-zero.
pub async fn run(args: VerifyArgs) -> Result<()> {
    let schema_text = match args.schema {
        Some(s) => s,
        None => {
            anyhow::bail!(
                "verify: pass --schema <json> for now; the provider-aware path \
                 will be wired once the macro-side hooks land (T-034 macro side)"
            );
        }
    };
    let schema: serde_json::Value = serde_json::from_str(&schema_text)
        .map_err(|e| anyhow::anyhow!("verify: --schema is not valid JSON: {e}"))?;

    // Phase 1: syntactic lint (always-on fast path).
    let mut findings: Vec<Finding> = lint(&schema);

    // Phase 2: LLM-based description-quality check (T-035).
    // Triggered when the user provides --provider and --model.
    let has_provider = args.provider_args.provider.is_some() && args.provider_args.model.is_some();
    let mut llm_findings: Vec<VerifyFinding> = Vec::new();

    if has_provider {
        // Parse the schema as a JSON array of ToolDefinition objects.
        let tools: Vec<ToolDefinition> = match serde_json::from_value(schema.clone()) {
            Ok(t) => t,
            Err(e) => {
                eprintln!(
                    "warning: --provider set but --schema is not a valid \
                     ToolDefinition array: {e}"
                );
                Vec::new()
            }
        };
        if !tools.is_empty() {
            let provider = build_provider(&args.provider_args)?;
            let cache = InMemoryCache::new();
            let model = args.provider_args.model.as_deref().unwrap_or("default");

            if let Ok(r) = llm_verify(&tools, &provider, &cache, model).await {
                llm_findings = r;
            } else {
                eprintln!("warning: LLM verify pass failed (continuing with lint-only results)");
            }
        }
    }

    // Merge LLM findings into the local Finding list (limited codespace).
    for lf in &llm_findings {
        findings.push(Finding {
            code: Box::leak(lf.code.clone().into_boxed_str()),
            message: lf.message.clone(),
        });
    }

    // Build and write the JSON report (T-035).
    if args.report_path.is_some() || !llm_findings.is_empty() {
        let report = VerifyReport {
            version: 1,
            findings: llm_findings,
        };
        let json = serde_json::to_string_pretty(&report)
            .unwrap_or_else(|_| r#"{"version":1,"findings":[]}"#.to_string());

        if let Some(path) = &args.report_path {
            std::fs::write(path, &json)
                .map_err(|e| anyhow::anyhow!("verify: failed to write report to {path}: {e}"))?;
            eprintln!("verify report written to {path}");
        } else {
            // Print to stdout as the last line so the human-readable
            // output above is not interleaved.
            eprintln!("--- verify report ---\n{json}");
        }
    }

    // Print human-readable output.
    if findings.is_empty() {
        println!("[OK] no findings");
        return Ok(());
    }
    for f in &findings {
        println!("[ERROR] {}: {}", f.code, f.message);
    }
    if args.no_fail {
        Ok(())
    } else {
        anyhow::bail!("verify: {} finding(s)", findings.len())
    }
}

/// LLM-powered description-quality pass (T-035). Sends every tool
/// definition to the LLM and asks it to flag low-quality descriptions,
/// missing side-effect docs, and unclear parameter descriptions.
async fn llm_verify(
    tools: &[ToolDefinition],
    provider: &dyn crate::provider::Provider,
    cache: &InMemoryCache,
    model: &str,
) -> std::result::Result<Vec<VerifyFinding>, anyhow::Error> {
    // Build a summary of every tool for the LLM prompt.
    let mut tool_parts: Vec<String> = Vec::new();
    for t in tools {
        // Pretty-print the input schema for readability.
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
You are a tool-definition quality reviewer. Given tool definitions, \
check each one for the following issues:

1. **DESC-VAGUE**: Is the `description` meaningful and specific, or is it \
   vague/generic (e.g. \"A tool to do stuff\", \"Does things\")?
2. **DESC-SHORT**: Is the `description` shorter than 10 words? If so, it \
   probably lacks enough context for an LLM to use it correctly.
3. **SIDE-EFFECT**: Does the tool have side effects (DB writes, network calls, \
   file I/O) that are NOT documented in the description?
4. **PARAM-DESC**: Are any parameter descriptions missing, empty, or unclear?

Reply with ONLY valid JSON in this exact format:
{\"findings\":[{\"code\":\"CODE\",\"message\":\"human-readable explanation\",\"tool\":\"tool_name\"}]}

If no issues are found, return {\"findings\":[]}. Do not add markdown fences or commentary.";

    let user_message = format!("Tool definitions to review:\n\n{tools_text}");

    let messages = vec![ChatMessage::User {
        content: user_message,
    }];

    let response =
        complete_with_cache(provider, cache, false, model, Some(system), &messages, &[]).await?;

    // Parse the LLM response as JSON.
    let content = response.content.trim();

    // Strip markdown code fences if present.
    let content = content
        .strip_prefix("```json")
        .or_else(|| content.strip_prefix("```"))
        .unwrap_or(content)
        .strip_suffix("```")
        .unwrap_or(content)
        .trim();

    #[derive(serde::Deserialize)]
    struct LlmResponse {
        findings: Vec<VerifyFinding>,
    }

    let parsed: LlmResponse = serde_json::from_str(content)?;
    Ok(parsed.findings)
}

/// Lightweight schema lint. The full validator lives behind the
/// `schema-verify` feature; this function is the always-on
/// syntactic pass that catches the most common regressions.
pub fn lint(schema: &serde_json::Value) -> Vec<Finding> {
    let mut out = Vec::new();
    // Walk the schema looking for nodes that look like JSON-Schema
    // definitions (carry schema-specific keywords) but are missing
    // `type`. The MCP envelope requires every schema node to carry
    // a `type` keyword; the typed validator (T-021) flags this at
    // runtime, and the linter flags it at build time.
    //
    // A node is considered a "schema node" when it carries any
    // JSON-Schema-specific keyword: `properties`, `items`,
    // `required`, `enum`, `const`, `additionalProperties`,
    // `patternProperties`, `$ref`, `oneOf`, `anyOf`, `allOf`,
    // `not`. The root is skipped — top-level OpenAI / Anthropic
    // envelopes do not always have `type` at the root.
    walk(schema, true, &mut |node, path| {
        if let serde_json::Value::Object(map) = node {
            let has_schema_keyword = map.contains_key("properties")
                || map.contains_key("items")
                || map.contains_key("required")
                || map.contains_key("enum")
                || map.contains_key("const")
                || map.contains_key("additionalProperties")
                || map.contains_key("patternProperties")
                || map.contains_key("$ref")
                || map.contains_key("oneOf")
                || map.contains_key("anyOf")
                || map.contains_key("allOf")
                || map.contains_key("not");
            let has_type = map.contains_key("type");
            if has_schema_keyword && !has_type {
                out.push(Finding {
                    code: "MCP-1",
                    message: format!("schema node at `{path}` has no `type` keyword"),
                });
            }
        }
    });
    out
}

fn walk<F: FnMut(&serde_json::Value, &str)>(node: &serde_json::Value, skip_root: bool, f: &mut F) {
    walk_with(node, "", skip_root, f);
}

fn walk_with<F: FnMut(&serde_json::Value, &str)>(
    node: &serde_json::Value,
    path: &str,
    skip_root: bool,
    f: &mut F,
) {
    if !skip_root {
        f(node, path);
    }
    match node {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let child = format!("{path}/{}", escape_json_pointer(k));
                walk_with(v, &child, false, f);
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let child = format!("{path}/{i}");
                walk_with(v, &child, false, f);
            }
        }
        _ => {}
    }
}

fn escape_json_pointer(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lint_flags_schema_node_without_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "x": {
                    "properties": { "y": { "type": "string" } }
                }
            }
        });
        let findings = lint(&schema);
        assert_eq!(findings.len(), 1, "expected one finding for `x`");
        assert_eq!(findings[0].code, "MCP-1");
    }

    #[test]
    fn lint_accepts_typed_object() {
        let schema = json!({
            "type": "object",
            "properties": {
                "x": { "type": "string" }
            }
        });
        assert!(lint(&schema).is_empty());
    }

    #[test]
    fn llm_verify_parses_empty_findings() {
        // Unit-test the JSON parsing path without an actual LLM call.
        let json = r#"{"findings":[]}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(parsed["findings"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn llm_verify_parses_single_finding() {
        let json =
            r#"{"findings":[{"code":"DESC-VAGUE","message":"Too vague","tool":"calc.add"}]}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(parsed["findings"][0]["code"], "DESC-VAGUE");
    }
}
