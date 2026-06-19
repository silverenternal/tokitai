//! T-019: `#[tool(result_truncate_bytes = 0)]` is a compile
//! error. The truncation sentinel would consume the whole
//! output (the truncation happens at byte 0, leaving only the
//! `...[truncated at 0 bytes, original was N bytes]` suffix),
//! so the parser rejects the literal at attribute-parse time
//! and `compile_error!` is emitted at the offending ident.

use tokitai::tool;

#[derive(Default)]
pub struct ZeroBudgetTools;

#[tool]
impl ZeroBudgetTools {
    /// `result_truncate_bytes = 0` is rejected because the
    /// truncation sentinel would consume the whole output.
    #[tool(result_truncate_bytes = 0)]
    pub fn bad(&self) -> String {
        "anything".to_string()
    }
}

fn main() {}
