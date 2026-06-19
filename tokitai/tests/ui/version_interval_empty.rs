// T-020: trybuild fixture for the empty-interval compile error.
//
// `since = "2.0.0"` is NOT strictly before `until = "2.0.0"`, so
// the dispatcher would never serve this method. The macro must
// refuse to compile and emit a `compile_error!` anchored at the
// offending method ident.

use tokitai::tool;

pub struct EmptyInterval;

#[tool(version_policy = "semver")]
impl EmptyInterval {
    #[tool(since = "2.0.0", until = "2.0.0")]
    pub fn stuck(&self) -> i64 {
        0
    }
}

fn main() {}