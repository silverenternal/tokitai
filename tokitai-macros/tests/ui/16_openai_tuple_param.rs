//! T-012: Negative fixture — OpenAI strict-mode rejects
//! positional tuples.
//!
//! OpenAI strict-mode does not implement JSON Schema 2020-12
//! `prefixItems` (positional tuples), so a Rust tuple parameter
//! is rejected with `E0030 [OA-3]` when the user opts into
//! the strict dialect.

use tokitai::tool;

pub struct TupleParam;

#[tool(dialect = "openai-strict")]
impl TupleParam {
    /// Pass a tuple — OpenAI strict rejects this.
    pub fn pass_tuple(&self, pair: (i32, i32)) -> i32 {
        pair.0 + pair.1
    }
}

fn main() {}