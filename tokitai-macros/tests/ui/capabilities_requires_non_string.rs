//! T-023: negative trybuild fixture for the `requires = [...]`
//! parser. The array contains a non-string entry (an integer
//! literal), which the parser rejects at compile time. The
//! trybuild snapshot (see
//! `capabilities_requires_non_string.stderr`) pins the
//! diagnostic so a future rustc version's wording change is
//! visible to the maintainer.

use tokitai::tool;

#[derive(Default)]
pub struct BadRequires;

#[tool]
impl BadRequires {
    /// Read sales data with a non-string `requires` entry.
    /// The macro pipeline should refuse to expand this impl
    /// block and emit a `compile_error!` diagnostic anchored at
    /// the offending array literal.
    #[tool(
        desc = "Read sales data for a given region string. Returns a JSON array of sales records, ordered by id ascending.",
        requires = ["db:read:sales", 42]
    )]
    pub fn read_sales(&self, region: String) -> String {
        format!("sales for {}", region)
    }
}

fn main() {
    // Unreachable at runtime — the macro refuses to compile.
    let _ = BadRequires::default();
}
