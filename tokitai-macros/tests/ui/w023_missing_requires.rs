//! T-023 / W023: a `#[tool]` impl block where a pub method omits
//! `requires = [...]`. The macro emits a `W023` warning via
//! `eprintln!` and generates the per-method CAPABILITIES const as
//! an empty slice.  When the method IS `pub` (so the macro
//! recognises it as a tool method) but has no `requires = [...]`,
//! the warning fires and the `CapabilityManifestProvider` impl
//! compiles with an empty aggregated manifest.
//!
//! This fixture must compile clean (zero errors) — W023 is a
//! warning, not a hard error.  Run with
//! `TOKITAI_TEST_FORCE_WARNINGS=1` to capture the W023 line in
//! the `.stderr` snapshot:
//!   TOKITAI_TEST_FORCE_WARNINGS=1 TRYBUILD=overwrite \
//!     cargo test -p tokitai-macros --test ui_tests test_w023_missing_requires

#![allow(dead_code)]

use tokitai::tool;

#[derive(Default)]
pub struct MissingRequires;

#[tool]
impl MissingRequires {
    /// Multiply two integers.  The method is `pub` so the macro
    /// recognises it as a tool method, but it has no
    /// `requires = [...]` — the macro emits a `W023` warning
    /// (when warnings are enabled) and generates the
    /// per-method CAPABILITIES const as an empty slice.
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}

fn main() {
    let t = MissingRequires::default();
    let _ = t.multiply(2, 3);
}
