//! T-034: `tokitai-llm verify` subcommand.
//!
//! Lints a JSON-Schema (or a `ToolDefinition`-derived envelope) for
//! the same footguns the MCP typed layer (T-021) catches at runtime:
//! missing `type` keyword, `additionalProperties: true` when the
//! caller expected `false`, etc. The actual JSON-Schema validator
//! is gated on the `schema-verify` feature; without it, only a
//! lightweight syntactic check is performed.

use crate::cli::VerifyArgs;
use crate::Result;

/// One verifier finding. The CLI prints one line per finding
/// (`[ERROR] code: message`).
#[derive(Debug, Clone)]
pub struct Finding {
    /// Stable error code (e.g. `MCP-1` for "missing `type` keyword").
    pub code: &'static str,
    /// Human-readable description of the issue.
    pub message: String,
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

    let findings = lint(&schema);
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
        // The root has `type`, so the root is fine. The `properties`
        // value is `{x: {...}}` which is not a schema node (it is a
        // map of property names to schemas), so no finding. To get a
        // finding we need a sub-schema that is missing `type` but
        // looks like a schema (e.g. carries `properties`).
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
}
