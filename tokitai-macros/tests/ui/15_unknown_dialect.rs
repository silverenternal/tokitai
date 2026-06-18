//! T-012: Negative fixture — unknown dialect name.
//!
//! `#[tool(dialect = "garbage")]` is rejected at compile time
//! with `E0030` because `garbage` is not in the closed set of
//! known dialects.

use tokitai::tool;

pub struct UnknownDialect;

#[tool(dialect = "garbage")]
impl UnknownDialect {
    /// Echo a string back.
    pub fn echo(&self, message: String) -> String {
        message
    }
}

fn main() {}