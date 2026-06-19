//! T-023: positive trybuild fixture for the `requires = [...]`
//! parser. The array contains only string literals; the macro
//! pipeline should accept this and emit the
//! `CAPABILITIES_<NAME>` const plus the aggregated
//! `CAPABILITIES` slice.

use tokitai::tool;
use tokitai::ToolProvider;
use tokitai_core::CapabilityManifestProvider;

#[derive(Default)]
pub struct GoodRequires;

#[tool]
impl GoodRequires {
    /// Read sales data with a valid string-only `requires`
    /// entry. The macro pipeline should accept this and
    /// emit the per-method `CAPABILITIES_READ_SALES` const
    /// and the aggregated `CAPABILITIES` slice.
    #[tool(
        desc = "Read sales data for a given region string. Returns a JSON array of sales records, ordered by id ascending.",
        requires = ["db:read:sales"]
    )]
    pub fn read_sales(&self, region: String) -> String {
        format!("sales for {}", region)
    }

    /// Send an email. The blast radius covers both the
    /// database read and the SMTP egress.
    #[tool(
        desc = "Send an email to a customer using the SMTP egress. The subject string and body string are the email content. Returns a confirmation message string.",
        requires = ["db:read:sales", "net:egress:smtp"]
    )]
    pub fn send_email(&self, subject: String, _body: String) -> String {
        format!("email sent: {}", subject)
    }
}

fn main() {
    let _ = GoodRequires::default();
    // The aggregated slice is reachable through the trait.
    let manifest = <GoodRequires as CapabilityManifestProvider>::capability_manifest();
    assert_eq!(manifest.len(), 2);
    // The per-method consts are reachable directly.
    assert_eq!(GoodRequires::CAPABILITIES_READ_SALES, &["db:read:sales"]);
    // ToolProvider's `tool_definitions` is also reachable so
    // the fixture exercises the full T-023 surface.
    let _ = <GoodRequires as ToolProvider>::tool_definitions();
}
