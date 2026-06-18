//! Test 14: Invalid return type - should compile fail
//!
//! Exercises T-001: the diagnostic for an unsupported return
//! type (here a bare function pointer) must surface at the
//! user-written method name (line 11, col 12), not at the
//! `#[tool]` attribute on line 8. The previous behaviour
//! reported the error at the macro call site, forcing users
//! to read `cargo expand` to find the offending method.

use tokitai::tool;

#[derive(Default)]
pub struct InvalidReturnTypeTools;

#[tool]
impl InvalidReturnTypeTools {
    /// Returning a bare function pointer is not schemable.
    pub fn fn_ptr_method(&self) -> fn(i32) -> i32 {
        |x| x
    }
}

// Note: the macro must reject this with E0021 (or an equivalent
// E0xxx code) anchored at line 11, col 12 (the method name), not
// at line 8 (the `#[tool]` attribute).
fn main() {}
