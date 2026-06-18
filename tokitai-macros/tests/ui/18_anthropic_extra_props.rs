//! T-012: Negative fixture — Anthropic flags a nested object
//! without explicit `additionalProperties: false`.
//!
//! The Anthropic parser requires every nested object that
//! declares `properties` to also declare
//! `additionalProperties: false`. The macro's codegen emits
//! an empty `Object` (without `additionalProperties`) for
//! user-defined struct parameters whose fields are not
//! visible at proc-macro time, and that empty object
//! triggers the `E0030 [AN-1]` audit rule.

use serde::Deserialize;
use tokitai::tool;

/// A user-defined struct. The macro does not know its
/// fields at expansion time, so the codegen emits an
/// empty `Object` schema (no `properties`, no
/// `additionalProperties` declaration).
#[derive(Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub age: u32,
}

pub struct ExtraProps;

#[tool(dialect = "anthropic")]
impl ExtraProps {
    /// Pass a user object — Anthropic rejects this because
    /// the rendered schema for `UserInfo` is an empty
    /// `Object` without an explicit `additionalProperties:
    /// false` declaration.
    pub fn pass_user(&self, user: UserInfo) -> String {
        user.name
    }
}

fn main() {}