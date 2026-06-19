// T-020: trybuild fixture for an invalid SemVer literal under
// `version_policy = "semver"`. The user wrote CalVer, which is
// not SemVer; the macro must refuse to compile with a
// `compile_error!` recommending they drop the policy.

use tokitai::tool;

pub struct CalVerTools;

#[tool(version_policy = "semver")]
impl CalVerTools {
    #[tool(since = "2026.06")]
    pub fn june(&self) -> i64 {
        6
    }
}

fn main() {}