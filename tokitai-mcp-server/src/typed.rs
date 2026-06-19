//! T-021: Typed MCP handle layer.
//!
//! # Defense-in-depth against the CVE-2025-59377 class of MCP vulnerabilities
//!
//! CVE-2025-59377 (mcp-kubernetes-server, 2025-09-25) and the broader family
//! of "tool handler passes unvalidated JSON straight to `subprocess.run(...,
//! shell=True)`" disclosures share one architectural trait: **the JSON schema
//! is advertised in `tools/list`, but the handler never enforces it**. A
//! malicious or simply buggy LLM caller can supply a string where the schema
//! says integer, or an object where the schema says string, and the handler
//! concatenates it into a shell command. The shell metacharacter wins; the
//! type mismatch is irrelevant because the schema was never checked.
//!
//! This module sits between the wire-level transport (T-005 stdio / HTTP)
//! and the handler. For every tool listed in `tools/list`, we read its
//! `inputSchema` and **validate the caller's arguments against the schema
//! before the handler ever runs**. If validation fails, we return a
//! `ToolError` of kind `ValidationError` with the JSON Pointer to the
//! offending field in the error message. The handler is not invoked; no
//! shell, no eval, no subprocess.
//!
//! ## Threat model
//!
//! What this layer catches:
//! - Wrong JSON type for a property (string supplied where schema says integer).
//! - Missing required property.
//! - Unexpected extra property when `additionalProperties: false` is set.
//! - Numeric out-of-range when `minimum` / `maximum` are set.
//! - String length out-of-range when `minLength` / `maxLength` are set.
//! - Wrong root type (`tools/call` `arguments` not an object).
//!
//! What this layer does NOT catch (deliberately scoped out of T-021):
//! - Semantic validation (e.g. "user_id 0 is logically nonsense").
//! - Side-channel validation (handler internal state).
//! - Authorization (handled at the auth layer above us).
//!
//! ## Feature gate
//!
//! This module is always compiled (it has no dependencies on `rmcp` or any
//! MCP SDK; it uses only `serde_json` + `tokitai_core`), but the typed
//! dispatch path is wired in only when the consumer enables the
//! `mcp-typed` feature. With the feature OFF the behavior is identical to
//! the T-005 JSON-passthrough path — no extra validation runs, no extra
//! allocations, no behavioral difference. That guarantees backward
//! compatibility.
//!
//! ## Backward compatibility
//!
//! - `cargo build --no-default-features` (the T-005 path): identical
//!   behavior; this module is compiled but unused.
//! - `cargo build --features mcp-typed`: every `tools/call` validates
//!   against the fixture's `inputSchema` before the handler runs.
//!
//! ## No `rmcp` dependency
//!
//! Hard rule reaffirmed: `Cargo.toml` lists no `rmcp` and no MCP SDK. This
//! module is implemented on top of `serde_json::Value` and the JSON-Schema
//! subset that the project's fixtures actually use (see
//! `tests/fixtures/mcp-spec/typed/*.json`). Adding a JSON-Schema dependency
//! would violate the project's "no second MCP SDK" principle and is
//! deliberately avoided.
//!
//! ## Reference: CVE-2025-59377
//!
//! `mcp-kubernetes-server` through 0.1.11 allows OS command injection via
//! the `/mcp/kubectl` endpoint: it executes the supplied command with
//! `subprocess` using `shell=True`, enabling injection through shell
//! metacharacters. The schema validation (if any) happens AFTER the sink.
//! This module's contract — validate BEFORE the handler — is exactly the
//! defense that CVE-2025-59377 demonstrates is missing from the ecosystem.
//!
//! ## CVE-2025-59377 → T-021 mapping
//!
//! - CVE sink: `subprocess.run(..., shell=True)`.
//! - T-021 guard: [`validate_against_schema`] refuses any call that does
//!   not match the JSON type and constraint declared in the fixture. The
//!   handler is unreachable from a malformed call.
//!
//! See `docs/MCP_ARCHITECTURE.md` § "Typed handle layer (T-021)" for the
//! user-facing overview.

use serde_json::Value;
use tokitai_core::{ToolError, ToolErrorKind};

/// A JSON Pointer (RFC 6901) fragment identifying where in the input a
/// validation error was detected. The empty string means "root".
pub type JsonPointer = String;

/// Validate a JSON-Schema subset of a `tools/call` arguments object.
///
/// # Arguments
///
/// * `schema` — the tool's `inputSchema` (a JSON-Schema fragment as
///   serialized in `tools/list`). Only the fields the project's fixtures
///   exercise are enforced: `type`, `properties`, `required`,
///   `additionalProperties`, `minimum`, `maximum`, `minLength`, `maxLength`.
/// * `args` — the caller-supplied arguments value.
///
/// # Returns
///
/// `Ok(())` when `args` satisfies the schema; otherwise a
/// [`ToolError`] of kind [`ToolErrorKind::ValidationError`] whose
/// `message` includes the JSON Pointer to the offending field.
///
/// # Errors
///
/// Returns `ToolError::ValidationError` for every malformed input.
/// Specifically:
///
/// - `args` is not an object.
/// - A `required` property is missing.
/// - A property's type does not match the schema's `type`.
/// - A numeric property is outside `[minimum, maximum]`.
/// - A string property's length is outside `[minLength, maxLength]`.
/// - An extra property is present and `additionalProperties == false`.
///
/// # Why this returns `ToolError`
///
/// `ToolError::ValidationError` is the canonical pre-handler error in
/// Tokitai. The macro-generated `__call_*` wrappers return this same
/// kind for type-mismatch failures, so consumers see one consistent
/// error type regardless of whether the failure was at the macro layer
/// (compile-time schema) or at the typed layer (runtime schema, when
/// `inputSchema` lives in an MCP fixture).
pub fn validate_against_schema(schema: &Value, args: &Value) -> Result<(), ToolError> {
    let mut path: Vec<String> = Vec::new();
    validate_node(schema, args, &mut path)
}

fn push_segment(path: &mut Vec<String>, segment: String) {
    path.push(segment);
}

fn pop_segment(path: &mut Vec<String>) {
    path.pop();
}

fn pointer(path: &[String]) -> JsonPointer {
    if path.is_empty() {
        String::new()
    } else {
        let mut s = String::new();
        for seg in path {
            s.push('/');
            // RFC 6901: escape '~' as '~0' and '/' as '~1'.
            s.push_str(&seg.replace('~', "~0").replace('/', "~1"));
        }
        s
    }
}

fn err_at(path: &[String], msg: impl Into<String>) -> ToolError {
    let ptr = pointer(path);
    let message = if ptr.is_empty() {
        msg.into()
    } else {
        format!("at `{}`: {}", ptr, msg.into())
    };
    ToolError::new(ToolErrorKind::ValidationError, message)
}

fn validate_node(schema: &Value, value: &Value, path: &mut Vec<String>) -> Result<(), ToolError> {
    // If schema has no `type`, accept anything.
    let schema_type = schema.get("type").and_then(|v| v.as_str());

    // Some fixtures use `{"$schema": "...", "type": "object", ...}`.
    // We honor `type` when present; otherwise we walk `properties` if
    // there are any.
    match schema_type {
        Some("object") => validate_object(schema, value, path),
        Some("integer") => validate_integer(schema, value, path),
        Some("number") => validate_number(schema, value, path),
        Some("string") => validate_string(schema, value, path),
        Some("boolean") => validate_boolean(value, path),
        Some("array") => validate_array(schema, value, path),
        Some("null") => validate_null(value, path),
        Some(other) => {
            // Unknown type — be conservative; refuse.
            Err(err_at(
                path,
                format!("schema declares unknown type `{}`", other),
            ))
        }
        None => {
            // No `type`. If we have `properties`, treat as object; else accept.
            if schema.get("properties").is_some() || schema.get("required").is_some() {
                validate_object(schema, value, path)
            } else {
                Ok(())
            }
        }
    }
}

fn validate_object(schema: &Value, value: &Value, path: &mut Vec<String>) -> Result<(), ToolError> {
    let obj = value
        .as_object()
        .ok_or_else(|| err_at(path, format!("expected object, got {}", json_kind(value))))?;

    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        for req in required {
            let Some(req_name) = req.as_str() else {
                continue;
            };
            if !obj.contains_key(req_name) {
                return Err(err_at(
                    path,
                    format!("missing required property `{}`", req_name),
                ));
            }
        }
    }

    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (key, child_schema) in props {
            if let Some(child_value) = obj.get(key) {
                push_segment(path, key.clone());
                let r = validate_node(child_schema, child_value, path);
                pop_segment(path);
                r?;
            }
        }
    }

    // `additionalProperties: false` rejects any extra key not in `properties`.
    if let Some(extra) = schema.get("additionalProperties").and_then(|v| v.as_bool()) {
        if !extra {
            if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
                for key in obj.keys() {
                    if !props.contains_key(key) {
                        return Err(err_at(path, format!("unexpected property `{}`", key)));
                    }
                }
            } else {
                // `additionalProperties: false` with no `properties` means
                // no keys at all.
                if !obj.is_empty() {
                    return Err(err_at(path, "object must be empty"));
                }
            }
        }
    }

    Ok(())
}

fn validate_integer(schema: &Value, value: &Value, path: &[String]) -> Result<(), ToolError> {
    let n = value
        .as_i64()
        .ok_or_else(|| err_at(path, format!("expected integer, got {}", json_kind(value))))?;
    check_numeric_bounds(schema, n as f64, path)
}

fn validate_number(schema: &Value, value: &Value, path: &[String]) -> Result<(), ToolError> {
    let n = value
        .as_f64()
        .ok_or_else(|| err_at(path, format!("expected number, got {}", json_kind(value))))?;
    check_numeric_bounds(schema, n, path)
}

fn check_numeric_bounds(schema: &Value, n: f64, path: &[String]) -> Result<(), ToolError> {
    if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
        if n < min {
            return Err(err_at(
                path,
                format!("value {} is below minimum {}", n, min),
            ));
        }
    }
    if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
        if n > max {
            return Err(err_at(
                path,
                format!("value {} is above maximum {}", n, max),
            ));
        }
    }
    Ok(())
}

fn validate_string(schema: &Value, value: &Value, path: &[String]) -> Result<(), ToolError> {
    let s = value
        .as_str()
        .ok_or_else(|| err_at(path, format!("expected string, got {}", json_kind(value))))?;
    if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
        if (s.chars().count() as u64) < min {
            return Err(err_at(
                path,
                format!(
                    "string length {} is below minLength {}",
                    s.chars().count(),
                    min
                ),
            ));
        }
    }
    if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
        if (s.chars().count() as u64) > max {
            return Err(err_at(
                path,
                format!(
                    "string length {} is above maxLength {}",
                    s.chars().count(),
                    max
                ),
            ));
        }
    }
    Ok(())
}

fn validate_boolean(value: &Value, path: &[String]) -> Result<(), ToolError> {
    if !value.is_boolean() {
        return Err(err_at(
            path,
            format!("expected boolean, got {}", json_kind(value)),
        ));
    }
    Ok(())
}

fn validate_null(value: &Value, path: &[String]) -> Result<(), ToolError> {
    if !value.is_null() {
        return Err(err_at(
            path,
            format!("expected null, got {}", json_kind(value)),
        ));
    }
    Ok(())
}

fn validate_array(schema: &Value, value: &Value, path: &mut Vec<String>) -> Result<(), ToolError> {
    let arr = value
        .as_array()
        .ok_or_else(|| err_at(path, format!("expected array, got {}", json_kind(value))))?;
    if let Some(items_schema) = schema.get("items") {
        for (i, item) in arr.iter().enumerate() {
            push_segment(path, i.to_string());
            let r = validate_node(items_schema, item, path);
            pop_segment(path);
            r?;
        }
    }
    Ok(())
}

fn json_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Validate `arguments` for a specific tool whose `inputSchema` is held by
/// the caller. This is the dispatch entry point used by the
/// `mcp-typed`-gated HTTP / stdio paths.
///
/// # Arguments
///
/// * `tool_name` — the MCP `tools/list` tool name (used only to enrich the
///   error message).
/// * `input_schema` — the tool's `inputSchema` from `tools/list`.
/// * `arguments` — the caller-supplied arguments.
///
/// # Returns
///
/// `Ok(())` on success; `Err(ToolError::ValidationError)` on any malformed
/// input. The error's `message` is prefixed with the tool name and
/// contains the JSON Pointer to the offending field, so a downstream
/// log scraper can route the failure to the right LLM client.
pub fn validate_tool_args(
    tool_name: &str,
    input_schema: &Value,
    arguments: &Value,
) -> Result<(), ToolError> {
    match validate_against_schema(input_schema, arguments) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Re-prefix with the tool name so an LLM client sees
            // "tool `add` at `/a`: expected integer, got string".
            let prefixed = format!("tool `{}`: {}", tool_name, e.message);
            Err(ToolError::new(ToolErrorKind::ValidationError, prefixed))
        }
    }
}

/// Load every fixture in `tests/fixtures/mcp-spec/typed/` into a typed
/// dispatch table. Each fixture file is a JSON object with a top-level
/// `tool_name`, `description`, `input_schema`, and `output_schema`.
///
/// # Path resolution
///
/// The fixture directory is resolved relative to `CARGO_MANIFEST_DIR` so
/// the table works from `cargo test`, `cargo build`, and `cargo run`.
/// The directory is scanned at runtime (no `build.rs` / no
/// `include_str!`) so adding a new fixture requires no `Cargo.toml`
/// edit.
///
/// # Returns
///
/// A `Vec<TypedToolSpec>` in deterministic order (sorted by file name).
/// A consumer that wants a `HashMap<String, &TypedToolSpec>` can build
/// one in `O(n)` from this vector.
pub fn load_typed_fixtures() -> Vec<TypedToolSpec> {
    let dir = typed_fixture_dir();
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return out, // missing dir = no fixtures (e.g. published crate)
    };
    let mut paths: Vec<_> = entries
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    paths.sort();
    for path in paths {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                if let Some(spec) = TypedToolSpec::from_value(&value) {
                    out.push(spec);
                }
            }
        }
    }
    out
}

fn typed_fixture_dir() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR is set at compile time by Cargo.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(manifest)
        .join("tests")
        .join("fixtures")
        .join("mcp-spec")
        .join("typed")
}

/// A single tool's schema loaded from a fixture file.
#[derive(Debug, Clone)]
pub struct TypedToolSpec {
    /// The tool name (must match the `tools/list` `name` field).
    pub tool_name: String,
    /// Free-form description from the fixture.
    pub description: String,
    /// The full `input_schema` JSON object. Held as `serde_json::Value`
    /// so the validator sees the same constraints the fixture declared.
    pub input_schema: Value,
    /// The full `output_schema` JSON object, if present.
    pub output_schema: Option<Value>,
}

impl TypedToolSpec {
    /// Parse a fixture file's JSON content into a [`TypedToolSpec`].
    /// Returns `None` if the JSON is missing `tool_name` / `input_schema`.
    pub fn from_value(v: &Value) -> Option<Self> {
        let tool_name = v.get("tool_name")?.as_str()?.to_string();
        let description = v
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        let input_schema = v.get("input_schema")?.clone();
        let output_schema = v.get("output_schema").cloned();
        Some(Self {
            tool_name,
            description,
            input_schema,
            output_schema,
        })
    }

    /// Validate a candidate `arguments` value against this tool's
    /// `input_schema`. Convenience wrapper around [`validate_tool_args`].
    pub fn validate(&self, arguments: &Value) -> Result<(), ToolError> {
        validate_tool_args(&self.tool_name, &self.input_schema, arguments)
    }
}

/// The wire-level dispatcher used by the `mcp-typed` paths.
///
/// Holds a copy of every typed spec loaded from the fixture directory.
/// `dispatch` looks up the spec by tool name, validates the caller's
/// `arguments`, and only then invokes the user-supplied handler. If
/// validation fails, the handler is NOT invoked.
///
/// # Example
///
/// ```rust,ignore
/// use tokitai_mcp_server::typed::{TypedDispatcher, load_typed_fixtures};
/// use serde_json::json;
///
/// let dispatcher = TypedDispatcher::from_specs(load_typed_fixtures());
///
/// // Malformed call: `a` is a string, schema says integer.
/// let args = json!({ "a": "ten", "b": 2 });
/// let result = dispatcher.dispatch("add", &args, |_validated_args| {
///     // unreachable when validation fails
///     Ok(json!(12))
/// });
/// assert!(result.is_err());
/// ```
pub struct TypedDispatcher {
    specs: Vec<TypedToolSpec>,
}

impl TypedDispatcher {
    /// Build a dispatcher from a pre-loaded spec list (deterministic
    /// order is not required; lookup is linear in the number of tools,
    /// which is small for any real MCP server).
    pub fn from_specs(specs: Vec<TypedToolSpec>) -> Self {
        Self { specs }
    }

    /// Load every fixture in the standard location. Equivalent to
    /// `TypedDispatcher::from_specs(load_typed_fixtures())`.
    pub fn from_fixtures() -> Self {
        Self::from_specs(load_typed_fixtures())
    }

    /// Number of tools the dispatcher knows about.
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// True iff no tools are registered.
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// Iterate the loaded specs in their stored order.
    pub fn specs(&self) -> &[TypedToolSpec] {
        &self.specs
    }

    /// Look up a spec by tool name.
    pub fn find(&self, tool_name: &str) -> Option<&TypedToolSpec> {
        self.specs.iter().find(|s| s.tool_name == tool_name)
    }

    /// Validate `arguments` against the named tool's `input_schema` and,
    /// if valid, call `handler` with the same arguments. If validation
    /// fails, `handler` is NOT invoked and the error is returned.
    ///
    /// # Handler contract
    ///
    /// `handler` receives the JSON arguments by reference. It is the
    /// handler's responsibility to deserialize them into Rust types; this
    /// module deliberately keeps that step separate so the validator can
    /// reject malformed shapes BEFORE the handler is constructed (which
    /// matters for handlers that call `serde_json::from_value` and would
    /// otherwise surface an `InternalError` instead of the canonical
    /// `ValidationError`).
    pub fn dispatch<F>(
        &self,
        tool_name: &str,
        arguments: &Value,
        handler: F,
    ) -> Result<Value, ToolError>
    where
        F: FnOnce(&Value) -> Result<Value, ToolError>,
    {
        let spec = self.find(tool_name).ok_or_else(|| {
            ToolError::not_found(format!("no typed spec for tool `{}`", tool_name))
        })?;
        spec.validate(arguments)?;
        handler(arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn add_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "a": { "type": "integer", "minimum": -10, "maximum": 10 },
                "b": { "type": "integer" }
            },
            "required": ["a", "b"],
            "additionalProperties": false
        })
    }

    #[test]
    fn valid_object_passes() {
        assert!(validate_against_schema(&add_schema(), &json!({"a": 1, "b": 2})).is_ok());
    }

    #[test]
    fn missing_required_field_rejected() {
        let err = validate_against_schema(&add_schema(), &json!({"a": 1})).unwrap_err();
        assert_eq!(err.kind, ToolErrorKind::ValidationError);
        assert!(err.message.contains("missing required property `b`"));
    }

    #[test]
    fn wrong_type_rejected_with_pointer() {
        let err = validate_against_schema(&add_schema(), &json!({"a": "x", "b": 2})).unwrap_err();
        assert_eq!(err.kind, ToolErrorKind::ValidationError);
        assert!(
            err.message.contains("/a"),
            "error must include JSON Pointer `/a` to offending field: {}",
            err.message
        );
    }

    #[test]
    fn extra_property_rejected_when_additional_false() {
        let err =
            validate_against_schema(&add_schema(), &json!({"a": 1, "b": 2, "c": 3})).unwrap_err();
        assert_eq!(err.kind, ToolErrorKind::ValidationError);
        assert!(err.message.contains("unexpected property `c`"));
    }

    #[test]
    fn numeric_bounds_enforced() {
        let err = validate_against_schema(&add_schema(), &json!({"a": 999, "b": 2})).unwrap_err();
        assert_eq!(err.kind, ToolErrorKind::ValidationError);
        assert!(err.message.contains("above maximum"));
    }

    #[test]
    fn string_length_bounds_enforced() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "minLength": 1, "maxLength": 3 }
            },
            "required": ["name"],
            "additionalProperties": false
        });
        let err = validate_against_schema(&schema, &json!({"name": "abcd"})).unwrap_err();
        assert_eq!(err.kind, ToolErrorKind::ValidationError);
        assert!(err.message.contains("maxLength"));
    }

    #[test]
    fn dispatcher_does_not_invoke_handler_on_validation_error() {
        let dispatcher = TypedDispatcher::from_specs(vec![TypedToolSpec::from_value(&json!({
            "tool_name": "add",
            "description": "add",
            "input_schema": add_schema(),
            "output_schema": {"type": "integer"}
        }))
        .unwrap()]);

        let mut handler_called = 0;
        let result = dispatcher.dispatch("add", &json!({"a": "ten", "b": 2}), |_args| {
            handler_called += 1;
            Ok(json!(12))
        });
        assert!(result.is_err());
        assert_eq!(
            handler_called, 0,
            "handler must not be called on validation error"
        );
    }

    #[test]
    fn dispatcher_invokes_handler_on_valid_args() {
        let dispatcher = TypedDispatcher::from_specs(vec![TypedToolSpec::from_value(&json!({
            "tool_name": "add",
            "description": "add",
            "input_schema": add_schema(),
            "output_schema": {"type": "integer"}
        }))
        .unwrap()]);

        let mut handler_called = 0;
        let result = dispatcher.dispatch("add", &json!({"a": 1, "b": 2}), |args| {
            handler_called += 1;
            let a = args.get("a").and_then(|v| v.as_i64()).unwrap_or(0);
            let b = args.get("b").and_then(|v| v.as_i64()).unwrap_or(0);
            Ok(json!(a + b))
        });
        assert_eq!(handler_called, 1);
        assert_eq!(result.unwrap(), json!(3));
    }
}
