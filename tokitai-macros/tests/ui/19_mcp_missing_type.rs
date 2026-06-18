//! T-012: Negative fixture — MCP rejects a property with no
//! explicit type.
//!
//! `serde_json::Value` (without the `Option<...>` wrapper)
//! is rendered as the catch-all `Any` schema. The MCP
//! 2025-06-18 dialect requires every property to declare
//! an explicit JSON Schema `type`, so this is rejected
//! with `E0030 [MCP-1]`.

use serde_json::Value;
use tokitai::tool;

pub struct MissingType;

#[tool(dialect = "mcp")]
impl MissingType {
    /// Pass any payload — MCP-2025-06-18 rejects this
    /// because the schema has no explicit `type`.
    pub fn pass_any(&self, payload: Value) -> String {
        payload.to_string()
    }
}

fn main() {}