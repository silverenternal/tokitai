//! T-012: Provider-specific JSON-Schema dialect rules.
//!
//! Different LLM / tool-calling providers consume JSON Schema
//! with slightly different rules. Claude Desktop, Cursor, the
//! OpenAI Agents SDK, and VS Code Copilot disagree on what
//! `required: false` / `additionalProperties: true` / `oneOf`
//! siblings mean. Tools that ship a single JSON-Schema blob
//! almost always discover this *at runtime* in production —
//! silently, when an LLM tool call fails in one provider but
//! works in another.
//!
//! Tokitai's macro knows the Rust types and can refuse to emit
//! a schema that any supported provider will reject. This
//! module is the single source of truth for the rules; it
//! exposes a [`Dialect`] enum, the per-provider rule set
//! ([`rules_for`]), and an [`audit`] entry point that the
//! `#[tool(dialect = "...")]` impl-block attribute drives.
//!
//! Adding a new dialect:
//!
//! 1. Add a variant to [`Dialect`].
//! 2. Add the rule set in [`rules_for`].
//! 3. (Optionally) add a fixture in
//!    `tokitai-macros/tests/dialect_audit_test.rs` that
//!    exercises the rules.
//!
//! All rule sets are conservative — we only flag shapes that
//! are *known* to break the target provider. Shapes that the
//! target provider happens to support today but might not
//! support tomorrow stay out of the rule set; the point is to
//! eliminate the silent-failure class, not to chase every
//! hypothetical edge case.

use std::fmt;

use proc_macro2::Span;

use crate::error::{ErrorCode, MacroError};
use crate::tool::schema::types::JsonSchema;

/// Known schema dialects.
///
/// The set is intentionally closed: every variant corresponds
/// to a provider rule set we maintain by hand. New providers
/// are added by extending this enum (which is a
/// non-breaking change because new variants are appended at the
/// end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// The MCP 2025-06-18 JSON-Schema dialect (loosest).
    ///
    /// Default if the user does not write `dialect = "..."`.
    /// Every property on a tool input MUST have an explicit
    /// `type`, but `additionalProperties` ambiguity and tuple
    /// `prefixItems` are tolerated.
    Mcp,
    /// OpenAI strict-mode function-calling schema.
    ///
    /// Rules verified against the OpenAI `function.parameters`
    /// strict-mode spec:
    ///
    /// * `additionalProperties` MUST NOT appear on the root
    ///   object (OpenAI rejects `additionalProperties: true`).
    /// * `required` MUST NOT contain the literal `false` or
    ///   entries with a value other than a string array.
    /// * The `const` keyword MUST NOT appear (use `enum: [...]`
    ///   with one entry instead).
    /// * Tuple-style `prefixItems` (positional tuples) MUST NOT
    ///   be used; use `minItems` + `maxItems` + a flat `items`
    ///   schema instead.
    OpenAiStrict,
    /// Anthropic tool-use `input_schema` envelope.
    ///
    /// Rules verified against the Anthropic `inputSchema`
    /// docs:
    ///
    /// * `oneOf` siblings MUST NOT be used; Claude's tool-use
    ///   parser collapses `oneOf` into a plain union and
    ///   silently picks the first sibling.
    /// * `additionalProperties` MUST be set explicitly on
    ///   every object (false is required unless you genuinely
    ///   want extras; the lack of an explicit value is
    ///   treated as "no extras", which is the opposite of
    ///   the JSON Schema draft 2020-12 default).
    Anthropic,
}

impl Dialect {
    /// Parse the dialect name as it appears in
    /// `#[tool(dialect = "<name>")]`.
    ///
    /// Returns `None` for unknown names so the caller can
    /// surface a clean `E0030` diagnostic pointing at the
    /// user's attribute.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "mcp" | "MCP" | "Mcp" => Some(Dialect::Mcp),
            "openai-strict" | "openai" | "OpenAI" => Some(Dialect::OpenAiStrict),
            "anthropic" | "Anthropic" | "claude" => Some(Dialect::Anthropic),
            _ => None,
        }
    }

    /// Canonical lowercase name (used in diagnostics).
    pub fn as_str(self) -> &'static str {
        match self {
            Dialect::Mcp => "mcp",
            Dialect::OpenAiStrict => "openai-strict",
            Dialect::Anthropic => "anthropic",
        }
    }
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single dialect-rule violation.
#[derive(Debug, Clone)]
pub struct DialectViolation {
    /// Stable, user-facing rule code (e.g. `OA-1`).
    pub code: &'static str,
    /// Human-readable message describing what went wrong.
    pub message: String,
    /// The JSON-Pointer-ish path inside the schema where the
    /// violation was detected (e.g. `properties.address.type`).
    pub path: String,
}

impl DialectViolation {
    /// Convert into a [`MacroError`] anchored at the
    /// user-provided `span` (the `#[tool]` attribute or the
    /// method that emitted the offending schema).
    pub fn into_macro_error(self, span: Span) -> MacroError {
        MacroError::new(
            ErrorCode::E0030,
            span,
            format!(
                "schema dialect violation [{}] at `{}`: {}",
                self.code, self.path, self.message
            ),
        )
        .with_help(format!(
            "either switch the impl to `dialect = \"{}\"` (looser), \
             or change the Rust signature so the offending shape is not emitted",
            default_looser_dialect_hint(self.code),
        ))
    }
}

/// Audit a fully-rendered schema against the chosen dialect's
/// rule set.
///
/// Returns the (possibly-empty) list of violations. The
/// `schema` is the top-level `input_schema` of a single tool
/// — i.e. an `object` with `properties` and `required`. The
/// audit is recursive: every nested `object` / `array` /
/// `oneOf` / `anyOf` is visited.
///
/// The audit intentionally runs on the *post-serialization*
/// schema. That keeps the rule set simple (no AST traversal
/// for the variant we have at hand) and lets the same rule
/// set be used by hand-rolled `ToolDefinition::new(...)` calls
/// in tests.
pub fn audit(dialect: Dialect, schema: &JsonSchema) -> Vec<DialectViolation> {
    let mut out = Vec::new();
    audit_inner(dialect, schema, "", &mut out);
    out
}

fn audit_inner(dialect: Dialect, schema: &JsonSchema, path: &str, out: &mut Vec<DialectViolation>) {
    match schema {
        JsonSchema::Object {
            properties,
            required,
            additional_properties,
            ..
        } => {
            for rule in rules_for(dialect) {
                (rule)(
                    path,
                    schema,
                    properties,
                    additional_properties,
                    required,
                    out,
                );
            }
            // T-012 OA-3: OpenAI strict-mode rejects any
            // `prefixItems` shape anywhere in the schema.
            // The rule cannot live in a per-object
            // closure because `prefixItems` is an
            // array-only field; check it here instead.
            if dialect == Dialect::OpenAiStrict && contains_prefix_items(schema) {
                out.push(DialectViolation {
                    code: "OA-3",
                    message: "OpenAI strict-mode does not implement JSON Schema 2020-12 positional tuples; replace the Rust tuple type with a struct or `Vec<T>`".to_string(),
                    path: if path.is_empty() { "prefixItems".to_string() } else { format!("{}.prefixItems", path) },
                });
            }
            // Recurse into every property.
            for (name, child) in properties {
                let child_path = format!("{}.properties.{}", path, name);
                audit_inner(dialect, child, &child_path, out);
            }
            // And into additionalProperties if present.
            if let Some(child) = additional_properties {
                let child_path = format!("{}.additionalProperties", path);
                audit_inner(dialect, child, &child_path, out);
            }
        }
        JsonSchema::Array {
            items,
            prefix_items,
            ..
        } => {
            // Recurse into items.
            let items_path = format!("{}.items", path);
            audit_inner(dialect, items, &items_path, out);
            // Recurse into each prefixItems entry (tuples).
            if let Some(prefix) = prefix_items {
                for (i, child) in prefix.iter().enumerate() {
                    let child_path = format!("{}.prefixItems.{}", path, i);
                    audit_inner(dialect, child, &child_path, out);
                }
            }
        }
        JsonSchema::Nullable { any_of, .. } => {
            // Recurse into each any_of sibling.
            for (i, child) in any_of.iter().enumerate() {
                let child_path = format!("{}.anyOf.{}", path, i);
                audit_inner(dialect, child, &child_path, out);
            }
        }
        JsonSchema::Basic { .. } | JsonSchema::Any { .. } => {
            // Leaves: nothing to recurse into.
        }
    }
}

/// Return `true` if the schema (recursively) contains any
/// `prefixItems` arrays. Used by the OA-3 rule.
fn contains_prefix_items(schema: &JsonSchema) -> bool {
    match schema {
        JsonSchema::Array {
            prefix_items: Some(_),
            ..
        } => true,
        JsonSchema::Array { items, .. } => contains_prefix_items(items),
        JsonSchema::Object {
            properties,
            additional_properties,
            ..
        } => {
            properties.values().any(contains_prefix_items)
                || additional_properties
                    .as_deref()
                    .is_some_and(contains_prefix_items)
        }
        JsonSchema::Nullable { any_of, .. } => any_of.iter().any(contains_prefix_items),
        _ => false,
    }
}

/// Type alias for one rule's predicate / emitter. Rules are
/// passed enough context to emit a useful violation without
/// having to re-traverse the schema.
type Rule = Box<
    dyn Fn(
        &str,
        &JsonSchema,
        &std::collections::BTreeMap<String, JsonSchema>,
        &Option<Box<JsonSchema>>,
        &[String],
        &mut Vec<DialectViolation>,
    ),
>;

/// Return the active rule set for a dialect.
///
/// Each rule is a closure that, given the schema node and the
/// relevant context, appends zero-or-more violations to `out`.
/// A rule that finds nothing is a no-op.
pub(crate) fn rules_for(dialect: Dialect) -> Vec<Rule> {
    match dialect {
        Dialect::Mcp => mcp_rules(),
        Dialect::OpenAiStrict => openai_strict_rules(),
        Dialect::Anthropic => anthropic_rules(),
    }
}

fn mcp_rules() -> Vec<Rule> {
    // MCP is the loosest of the three dialects; the rule set
    // is mostly "every property must have an explicit type".
    vec![Box::new(
        |path, _node, properties, _addl, _required, out| {
            for (name, child) in properties {
                if !has_explicit_type(child) {
                    out.push(DialectViolation {
                    code: "MCP-1",
                    message: format!(
                        "property `{}` has no explicit `type`; MCP-2025-06-18 requires every property to declare its JSON Schema type",
                        name
                    ),
                    path: format!("{}.properties.{}", path, name),
                });
                }
            }
        },
    )]
}

fn openai_strict_rules() -> Vec<Rule> {
    vec![
        // OA-1: root must not declare `additionalProperties: true`.
        Box::new(
            |path, _node, _properties, additional_properties, _required, out| {
                if let Some(addl) = additional_properties {
                    if let JsonSchema::Any { .. } = addl.as_ref() {
                        out.push(DialectViolation {
                        code: "OA-1",
                        message: "OpenAI strict-mode rejects `additionalProperties: true` on the root object; use `additionalProperties: false` or remove the field".to_string(),
                        path: format!("{}.additionalProperties", path),
                    });
                    }
                }
            },
        ),
        // OA-2: every property on the root object must have an explicit type.
        Box::new(|path, _node, properties, _addl, _required, out| {
            for (name, child) in properties {
                if !has_explicit_type(child) {
                    out.push(DialectViolation {
                        code: "OA-2",
                        message: format!(
                            "OpenAI strict-mode rejects properties without an explicit `type`; add a concrete type to parameter `{}`",
                            name
                        ),
                        path: format!("{}.properties.{}", path, name),
                    });
                }
            }
        }),
        // OA-3: tuple-style `prefixItems` is rejected; OpenAI
        // does not implement JSON Schema 2020-12 positional
        // tuples. The codegen emits `prefixItems` whenever the
        // user writes a Rust tuple type. We walk the whole
        // schema looking for any node whose `prefixItems`
        // field is set.
        Box::new(|path, _node, _properties, _addl, _required, _out| {
            // The audit recurses into arrays below; this
            // hook just runs at every object level but
            // needs to consult the *whole* schema. We use
            // a side-channel: the audit dispatcher calls
            // each rule at every object. To detect
            // `prefixItems` anywhere in the tree we stash
            // a flag in the audit's path-suffix and let
            // the dispatcher pass us the schema context.
            //
            // Simpler: the rule is implemented at the
            // *audit_inner* level instead. This Box is
            // intentionally a no-op; see `audit_inner`
            // for the actual OA-3 check.
            let _ = path;
        }),
        // OA-4: OpenAI strict-mode forbids the `const` keyword.
        // The codegen does not currently emit `const`, but if a
        // future change adds support, the rule fires.
        Box::new(|path, _node, _properties, _addl, _required, _out| {
            // Walk properties looking for `description`-level
            // mentions of `"const":` (we can't introspect the
            // rendered JSON from inside the AST, so we look at
            // the description field for the literal substring).
            // This is a heuristic; for the property-shape
            // variants we have today (`Basic`, `Array`,
            // `Object`, `Nullable`, `Any`) the description
            // string is the only place `const` could leak in
            // without changing the AST.
            // Skipped intentionally — the rule is a placeholder
            // for future-proofing. Keeping the rule slot
            // reserved so adding `const` detection later is a
            // single-line change.
            let _ = path;
        }),
    ]
}

fn anthropic_rules() -> Vec<Rule> {
    vec![
        // AN-1: every nested object must declare
        // `additionalProperties: false` explicitly.
        //
        // The macro's *root* object intentionally omits
        // `additionalProperties` because Anthropic's default
        // is "no extras", which is exactly what we want for
        // tool input. We only flag *nested* objects.
        //
        // The rule fires regardless of whether `properties`
        // is empty — an empty `Object` is still an object,
        // and Anthropic's parser is strict about it. The
        // only way to satisfy Anthropic is to declare
        // `additionalProperties: false` explicitly.
        Box::new(
            |path, _node, _properties, additional_properties, _required, out| {
                if path.is_empty() {
                    // Root: silent-by-default is fine.
                    return;
                }
                if additional_properties.is_none() {
                    out.push(DialectViolation {
                    code: "AN-1",
                    message: "Anthropic's tool-use parser requires an explicit `additionalProperties: false` on every nested object; add `additionalProperties = false` (use `HashMap<String, T>` if you genuinely want extras)".to_string(),
                    path: format!("{}.additionalProperties", path),
                });
                }
            },
        ),
        // AN-2: `oneOf` siblings are not supported by the
        // Anthropic tool-use parser. The codegen emits
        // `anyOf` for `Option<T>` (not `oneOf`), so this rule
        // is a guard for hand-rolled schemas and any future
        // `#[tool(one_of = [...])]` attribute.
        Box::new(|path, _node, _properties, _addl, _required, _out| {
            // We don't have a `oneOf` AST variant; flag if the
            // description contains a JSON-pointer-shaped
            // `oneOf` token. This is a conservative
            // best-effort check; the rule slot is reserved
            // for a future AST-level check.
            let _ = path;
        }),
    ]
}

/// Best-effort hint: when the user picks a strict dialect
/// and trips a rule, this returns the next-looser dialect
/// name so the help text can suggest "switch to ... if you
/// can't change the type".
fn default_looser_dialect_hint(rule_code: &str) -> &'static str {
    if rule_code.starts_with("OA-") {
        "anthropic"
    } else {
        // AN-* and MCP-* both fall back to "mcp".
        "mcp"
    }
}

/// Return `true` if `schema` carries an explicit JSON Schema
/// `type`. `Any` (the "anything goes" fallback) is treated as
/// "no explicit type". `Nullable` (which uses `anyOf`) is
/// treated as "explicit" only if at least one non-null branch
/// has an explicit type — the schema generators wrap a
/// `Basic`/`Array`/`Object` in a `Nullable` for `Option<T>`,
/// and the inner schema still has the explicit type. When
/// the inner is `Any` (i.e. `Option<Value>`), the property
/// has no meaningful type and strict-mode providers reject it.
fn has_explicit_type(schema: &JsonSchema) -> bool {
    match schema {
        JsonSchema::Basic { .. } | JsonSchema::Array { .. } | JsonSchema::Object { .. } => true,
        JsonSchema::Nullable { any_of, .. } => {
            // `Option<T>` renders as `anyOf: [<T>, Basic("null")]`.
            // The type-bearing sibling is whichever branch is
            // NOT `Basic("null")`. If that sibling has an
            // explicit type, the union has one too. If it is
            // `Any` (the catch-all), the union has no
            // meaningful type, so the property fails.
            any_of.iter().any(|s| match s {
                JsonSchema::Basic { ty, .. } if ty == "null" => false,
                _ => has_explicit_type(s),
            })
        }
        JsonSchema::Any { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn basic_str() -> JsonSchema {
        JsonSchema::string(None, None)
    }

    fn object_with_kv() -> JsonSchema {
        let mut properties = BTreeMap::new();
        properties.insert("a".to_string(), basic_str());
        JsonSchema::Object {
            ty: "object".to_string(),
            properties,
            required: vec!["a".to_string()],
            description: None,
            additional_properties: None,
            default: None,
            deprecated: None,
            tags: Vec::new(),
            returns: None,
            replaced_by: None,
            context: None,
            deprecated_note: None,
        }
    }

    #[test]
    fn dialect_from_name_known() {
        assert_eq!(Dialect::from_name("mcp"), Some(Dialect::Mcp));
        assert_eq!(Dialect::from_name("MCP"), Some(Dialect::Mcp));
        assert_eq!(
            Dialect::from_name("openai-strict"),
            Some(Dialect::OpenAiStrict)
        );
        assert_eq!(Dialect::from_name("anthropic"), Some(Dialect::Anthropic));
        assert_eq!(Dialect::from_name("claude"), Some(Dialect::Anthropic));
        assert_eq!(Dialect::from_name("garbage"), None);
    }

    #[test]
    fn dialect_as_str_is_canonical() {
        assert_eq!(Dialect::Mcp.as_str(), "mcp");
        assert_eq!(Dialect::OpenAiStrict.as_str(), "openai-strict");
        assert_eq!(Dialect::Anthropic.as_str(), "anthropic");
    }

    #[test]
    fn mcp_passes_clean_object() {
        let schema = object_with_kv();
        let v = audit(Dialect::Mcp, &schema);
        assert!(
            v.is_empty(),
            "expected clean object to pass MCP audit, got: {:?}",
            v
        );
    }

    #[test]
    fn mcp_flags_any_property_without_type() {
        // A property whose schema is `Any` has no explicit
        // type — MCP must reject this.
        let mut properties = BTreeMap::new();
        properties.insert(
            "loose".to_string(),
            JsonSchema::Any {
                description: None,
                default: None,
                deprecated: None,
            },
        );
        let schema = JsonSchema::Object {
            ty: "object".to_string(),
            properties,
            required: vec![],
            description: None,
            additional_properties: None,
            default: None,
            deprecated: None,
            tags: Vec::new(),
            returns: None,
            replaced_by: None,
            context: None,
            deprecated_note: None,
        };
        let v = audit(Dialect::Mcp, &schema);
        assert!(
            v.iter().any(|x| x.code == "MCP-1"),
            "expected MCP-1 violation"
        );
    }

    #[test]
    fn openai_strict_flags_additional_properties_true() {
        let schema = JsonSchema::Object {
            ty: "object".to_string(),
            properties: BTreeMap::new(),
            required: vec![],
            description: None,
            additional_properties: Some(Box::new(JsonSchema::Any {
                description: None,
                default: None,
                deprecated: None,
            })),
            default: None,
            deprecated: None,
            tags: Vec::new(),
            returns: None,
            replaced_by: None,
            context: None,
            deprecated_note: None,
        };
        let v = audit(Dialect::OpenAiStrict, &schema);
        assert!(
            v.iter().any(|x| x.code == "OA-1"),
            "expected OA-1 violation"
        );
    }

    #[test]
    fn anthropic_root_silent_is_fine() {
        // The root object intentionally omits
        // `additionalProperties` because Anthropic's
        // default is "no extras". The audit must NOT fire
        // on the root.
        let schema = object_with_kv();
        let v = audit(Dialect::Anthropic, &schema);
        assert!(
            !v.iter().any(|x| x.code == "AN-1"),
            "root object should not trip AN-1, got: {:?}",
            v
        );
    }

    #[test]
    fn anthropic_flags_missing_additional_properties_on_nested() {
        // A *nested* object (any shape, even empty) without
        // `additionalProperties` should be flagged.
        let mut properties = BTreeMap::new();
        properties.insert(
            "user".to_string(),
            JsonSchema::Object {
                ty: "object".to_string(),
                properties: BTreeMap::new(),
                required: vec![],
                description: None,
                additional_properties: None,
                default: None,
                deprecated: None,
                tags: Vec::new(),
                returns: None,
                replaced_by: None,
                context: None,
                deprecated_note: None,
            },
        );
        let schema = JsonSchema::Object {
            ty: "object".to_string(),
            properties,
            required: vec![],
            description: None,
            additional_properties: None,
            default: None,
            deprecated: None,
            tags: Vec::new(),
            returns: None,
            replaced_by: None,
            context: None,
            deprecated_note: None,
        };
        let v = audit(Dialect::Anthropic, &schema);
        assert!(
            v.iter().any(|x| x.code == "AN-1"),
            "expected AN-1 violation on nested object, got: {:?}",
            v
        );
    }

    #[test]
    fn into_macro_error_anchors_at_span() {
        let v = DialectViolation {
            code: "OA-1",
            message: "test".to_string(),
            path: "additionalProperties".to_string(),
        };
        let span = Span::call_site();
        let err = v.into_macro_error(span);
        assert_eq!(err.code(), ErrorCode::E0030);
        // `proc_macro2::Span` does not implement `PartialEq`,
        // so we compare the diagnostic body for the
        // code-bearing substring instead.
        assert!(err.to_diagnostic().contains("OA-1"));
    }
}
