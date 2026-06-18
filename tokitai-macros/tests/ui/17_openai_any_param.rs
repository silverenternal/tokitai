//! T-012: Negative fixture — OpenAI strict-mode rejects
//! properties without an explicit type.
//!
//! `Option<serde_json::Value>` is rendered as a `Nullable`
//! whose inner is the catch-all `Any` schema (no explicit
//! type). OpenAI strict-mode requires every property to
//! declare a `type` and surfaces this as `E0030 [OA-2]`.

use serde_json::Value;
use tokitai::tool;

pub struct AnyParam;

#[tool(dialect = "openai-strict")]
impl AnyParam {
    /// Pass any payload — OpenAI strict rejects this
    /// because the schema has no explicit `type`.
    pub fn pass_any(&self, payload: Option<Value>) -> String {
        payload
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".to_string())
    }
}

fn main() {}