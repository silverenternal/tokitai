//! T-023: macro-level test for the `requires = [...]` attribute
//! parser, the `CAPABILITIES_<METHOD_NAME>` per-method const
//! emission, and the aggregated `CAPABILITIES` slice.

use tokitai::tool;
use tokitai_core::CapabilityManifestProvider;

#[derive(Default)]
pub struct RequiresTool;

#[tool]
impl RequiresTool {
    /// Read sales data for a region. The blast radius
    /// declaration makes the data access visible to the server
    /// at startup.
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

#[test]
fn aggregated_manifest_contains_every_method() {
    let manifest = <RequiresTool as CapabilityManifestProvider>::capability_manifest();
    assert_eq!(manifest.len(), 2);
    let by_name: std::collections::HashMap<&str, &[&str]> =
        manifest.iter().map(|(name, caps)| (*name, *caps)).collect();
    assert_eq!(by_name["read_sales"], &["db:read:sales"]);
    assert_eq!(by_name["send_email"], &["db:read:sales", "net:egress:smtp"]);
}

#[test]
fn per_method_consts_are_reachable() {
    // The per-method consts are reachable as
    // `RequiresTool::CAPABILITIES_READ_SALES` etc. We
    // assert their values match the user's declaration.
    assert_eq!(RequiresTool::CAPABILITIES_READ_SALES, &["db:read:sales"]);
    assert_eq!(
        RequiresTool::CAPABILITIES_SEND_EMAIL,
        &["db:read:sales", "net:egress:smtp"]
    );
}

#[test]
fn aggregated_const_matches_trait_method() {
    // The aggregated const and the trait method should
    // point at the same data (modulo the `&'static`
    // lifetime of the const slice).
    let trait_slice = <RequiresTool as CapabilityManifestProvider>::capability_manifest();
    assert_eq!(trait_slice.len(), RequiresTool::CAPABILITIES.len());
    for (a, b) in trait_slice.iter().zip(RequiresTool::CAPABILITIES.iter()) {
        assert_eq!(a.0, b.0);
        assert_eq!(a.1, b.1);
    }
}
