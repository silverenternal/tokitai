//! T-022: adversarial description lint — positive trybuild fixture.
//!
//! The canonical clean description from the T-022 acceptance
//! criteria in `todo.json` v3.0. The literal contains no
//! instruction-like phrase, no chat-template role header, no
//! fake-prompt break, no oversized narrative, and no user-
//! supplied blocklist entry. The macro pipeline should expand
//! the impl block without emitting any `compile_error!`.

use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct DescSafetyClean;

#[tool]
impl DescSafetyClean {
    #[tool(desc = "Adds two 32-bit integers and returns their sum as i32. Requires both operands to be in the i32 range; returns Err on overflow.")]
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

fn main() {
    let tools = DescSafetyClean::tool_definitions();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "add");
}

