//! Minimal OpenAPI 3.0 / 3.1 spec parser.
//!
//! Used by the `#[openapi(...)]` proc-macro to extract the operation
//! metadata (operationId, summary, description, request body schema,
//! parameters) it needs to synthesize `#[tool]`-equivalent definitions.
//!
//! This is intentionally a *lightweight* parser: it only understands the
//! fields the macro consumes. Unknown fields are tolerated (serde's
//! default behaviour) so a fully-featured OpenAPI document round-trips
//! losslessly.

#![allow(dead_code)]

use serde::Deserialize;
use std::collections::BTreeMap;

/// A parsed OpenAPI 3.x specification.
#[derive(Debug, Default, Deserialize)]
pub struct OpenApiSpec {
    /// `openapi` version string (e.g. `"3.0.3"` or `"3.1.0"`).
    #[serde(default)]
    pub openapi: String,
    /// Optional `info` block.
    #[serde(default)]
    pub info: Info,
    /// All paths declared in the spec, keyed by path template.
    #[serde(default)]
    pub paths: BTreeMap<String, PathItem>,
    /// Optional `components` block (schemas are kept as raw JSON so we
    /// don't need to model `$ref` resolution at proc-macro time).
    #[serde(default)]
    pub components: Option<Components>,
}

/// The OpenAPI `info` block.
#[derive(Debug, Default, Deserialize)]
pub struct Info {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// A single path item (one URL template) and its operations.
#[derive(Debug, Default, Deserialize)]
pub struct PathItem {
    #[serde(default)]
    pub get: Option<Operation>,
    #[serde(default)]
    pub post: Option<Operation>,
    #[serde(default)]
    pub put: Option<Operation>,
    #[serde(default)]
    pub delete: Option<Operation>,
    #[serde(default)]
    pub patch: Option<Operation>,
    #[serde(default)]
    pub options: Option<Operation>,
    #[serde(default)]
    pub head: Option<Operation>,
    #[serde(default)]
    pub trace: Option<Operation>,
}

impl PathItem {
    /// Iterate over every operation defined on this path item.
    pub fn operations(&self) -> impl Iterator<Item = &Operation> {
        self.get
            .iter()
            .chain(self.post.iter())
            .chain(self.put.iter())
            .chain(self.delete.iter())
            .chain(self.patch.iter())
            .chain(self.options.iter())
            .chain(self.head.iter())
            .chain(self.trace.iter())
    }
}

/// A single OpenAPI operation (i.e. one HTTP method on one path).
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Operation {
    /// OperationId — the canonical key the macro matches against.
    #[serde(default, rename = "operationId")]
    pub operation_id: Option<String>,
    /// Short summary.
    #[serde(default)]
    pub summary: Option<String>,
    /// Long description.
    #[serde(default)]
    pub description: Option<String>,
    /// `parameters` array.
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    /// `requestBody` object.
    #[serde(default, rename = "requestBody")]
    pub request_body: Option<RequestBody>,
    /// `responses` object, keyed by status code (e.g. `"200"`, `"default"`).
    #[serde(default)]
    pub responses: BTreeMap<String, Response>,
    /// HTTP method as a string (e.g. `"GET"`, `"POST"`). Populated by
    /// the iterator helper, not by deserialization.
    #[serde(skip)]
    pub http_method: Option<String>,
    /// URL path template (e.g. `/v1/chat/completions`). Populated by
    /// the iterator helper, not by deserialization.
    #[serde(skip)]
    pub path: Option<String>,
}

impl Operation {
    /// Return the first description-y string in `summary` / `description`,
    /// falling back to an empty string. `summary` is preferred because
    /// OpenAPI conventions make it a short, single-sentence description
    /// — which is exactly what an AI tool description should be.
    pub fn description_or_summary(&self) -> String {
        self.summary
            .clone()
            .or_else(|| self.description.clone())
            .unwrap_or_default()
    }

    /// Best-effort: produce a JSON Schema fragment that the macro can
    /// advertise as the tool's `input_schema`. The fragment is built
    /// from the operation's `requestBody.content["application/json"].schema`
    /// when available, otherwise from the merged `parameters` list, and
    /// finally a bare `{"type":"object"}` fallback.
    pub fn input_schema(&self) -> serde_json::Value {
        // 1. Prefer the request body schema (usually the most accurate).
        if let Some(body) = &self.request_body {
            if let Some(media) = body.content.get("application/json") {
                if let Some(schema) = &media.schema {
                    if let Some(obj) = schema.as_object() {
                        if obj.get("type").and_then(|v| v.as_str()) == Some("object")
                            || obj.contains_key("properties")
                        {
                            return schema.clone();
                        }
                    } else {
                        return schema.clone();
                    }
                }
            }
        }

        // 2. Fall back to a `{type:"object",properties:...,required:...}`
        //    built from the `parameters` array.
        if !self.parameters.is_empty() {
            let mut properties = serde_json::Map::new();
            let mut required: Vec<serde_json::Value> = Vec::new();
            for p in &self.parameters {
                let key = match &p.schema_name {
                    Some(n) => n.clone(),
                    None => continue,
                };
                let mut prop = p
                    .schema
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({"type": "string"}));
                if let Some(desc) = &p.description {
                    if let Some(obj) = prop.as_object_mut() {
                        obj.insert("description".to_string(), serde_json::Value::String(desc.clone()));
                    }
                }
                properties.insert(key.clone(), prop);
                if p.required {
                    required.push(serde_json::Value::String(key));
                }
            }
            return serde_json::json!({
                "type": "object",
                "properties": properties,
                "required": required,
            });
        }

        // 3. Last resort: a bare object schema.
        serde_json::json!({"type": "object"})
    }
}

/// A single `parameters` entry.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Parameter {
    /// Parameter name.
    pub name: String,
    /// Parameter location (`"query"`, `"path"`, `"header"`, `"cookie"`).
    #[serde(rename = "in", default)]
    pub location: Option<String>,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the parameter is required.
    #[serde(default)]
    pub required: bool,
    /// Inline schema fragment for the parameter.
    #[serde(default)]
    pub schema: Option<serde_json::Value>,

    // Convenience: normalised name for code generation.
    #[serde(skip)]
    pub schema_name: Option<String>,
}

impl Parameter {
    /// Re-key `name` into `schema_name` so the rest of the parser can
    /// treat path/query/header parameters uniformly.
    pub fn normalise(mut self) -> Self {
        self.schema_name = Some(self.name.clone());
        self
    }
}

/// `requestBody` object.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct RequestBody {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub content: BTreeMap<String, MediaType>,
    #[serde(default)]
    pub required: bool,
}

/// A single media-type entry inside a request body or response.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct MediaType {
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
}

/// A single `responses` entry.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Response {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub content: Option<BTreeMap<String, MediaType>>,
}

/// The `components` block — kept as a tree of `serde_json::Value` for
/// forward compatibility with `$ref` resolution.
#[derive(Debug, Default, Deserialize)]
pub struct Components {
    #[serde(default)]
    pub schemas: BTreeMap<String, serde_json::Value>,
}

impl OpenApiSpec {
    /// Parse a JSON string into an [`OpenApiSpec`].
    pub fn from_str(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Iterate over every operation in the spec. Each yielded
    /// `(path, method, &Operation)` carries the originating path
    /// template and HTTP method, which the macro needs to label the
    /// generated tool.
    pub fn all_operations(&self) -> Vec<(&str, &str, &Operation)> {
        let mut out = Vec::new();
        for (path, item) in &self.paths {
            for (method, op) in item.method_ops() {
                out.push((path.as_str(), method, op));
            }
        }
        out
    }

    /// Look up an operation by its `operationId`.
    pub fn lookup(&self, operation_id: &str) -> Option<(&str, &str, &Operation)> {
        for (path, item) in &self.paths {
            for (method, op) in item.method_ops() {
                if op.operation_id.as_deref() == Some(operation_id) {
                    return Some((path.as_str(), method, op));
                }
            }
        }
        None
    }
}

impl PathItem {
    /// Iterator over `(http_method, &Operation)` for this path item.
    pub fn method_ops(&self) -> impl Iterator<Item = (&'static str, &Operation)> {
        [
            ("GET", self.get.as_ref()),
            ("POST", self.post.as_ref()),
            ("PUT", self.put.as_ref()),
            ("DELETE", self.delete.as_ref()),
            ("PATCH", self.patch.as_ref()),
            ("OPTIONS", self.options.as_ref()),
            ("HEAD", self.head.as_ref()),
            ("TRACE", self.trace.as_ref()),
        ]
        .into_iter()
        .filter_map(|(m, opt): (&'static str, Option<&Operation>)| {
            opt.map(|op| (m, op))
        })
    }
}
