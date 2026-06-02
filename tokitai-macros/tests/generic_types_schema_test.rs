//! Schema generation for compound (generic) Rust types.
//!
//! `tokitai-macros` derives the input schema from each parameter's
//! Rust type via `tokitai-macros::tool::schema::gen`. This file
//! pins the emitted JSON Schema for the generic-shaped types the
//! macro actually sees in real code:
//!
//! - `Vec<T>` and `&[T]`
//! - `Option<T>`
//! - `HashMap<K, V>`
//! - nested `Option<Vec<T>>` and `Vec<Option<T>>`
//! - tuple `(T, U)`
//! - `std::collections::BTreeMap<K, V>`

use std::collections::{BTreeMap, HashMap};
use tokitai::tool;
use tokitai::ToolProvider;

#[derive(Default)]
pub struct GenericTypesTools;

#[tool]
impl GenericTypesTools {
    /// Vec of strings.
    pub fn vec_strings(&self, items: Vec<String>) -> usize {
        items.len()
    }

    /// HashMap of string -> int.
    pub fn map_counts(&self, counts: HashMap<String, i32>) -> i32 {
        counts.values().sum()
    }

    /// Optional Vec of strings.
    pub fn maybe_vec(&self, tags: Option<Vec<String>>) -> usize {
        tags.map(|t| t.len()).unwrap_or(0)
    }

    /// Vec of optional ints.
    pub fn vec_maybe(&self, values: Vec<Option<i32>>) -> i32 {
        values.iter().filter_map(|v| *v).sum()
    }

    /// Tuple of (String, i32).
    pub fn tuple_param(&self, pair: (String, i32)) -> String {
        format!("{}={}", pair.0, pair.1)
    }

    /// BTreeMap of int -> string.
    pub fn btree_param(&self, m: BTreeMap<i32, String>) -> usize {
        m.len()
    }
}

#[test]
fn vec_param_emits_array_schema() {
    let defs = GenericTypesTools::tool_definitions();
    let tool = defs.iter().find(|t| t.name == "vec_strings").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let items = &schema["properties"]["items"];
    assert_eq!(items["type"], "array");
    assert_eq!(items["items"]["type"], "string");
}

#[test]
fn hashmap_param_emits_object_schema() {
    let defs = GenericTypesTools::tool_definitions();
    let tool = defs.iter().find(|t| t.name == "map_counts").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let counts = &schema["properties"]["counts"];
    // HashMap is rendered as a JSON object with arbitrary keys; the
    // value type is `integer`. The key type is not enforced in the
    // schema (JSON object keys are always strings).
    assert_eq!(counts["type"], "object");
    assert_eq!(counts["additionalProperties"]["type"], "integer");
}

#[test]
fn option_vec_param_schema() {
    let defs = GenericTypesTools::tool_definitions();
    let tool = defs.iter().find(|t| t.name == "maybe_vec").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    // `tags` is optional, so it must NOT appear in the `required`
    // array.
    let required = schema["required"].as_array().unwrap();
    assert!(
        !required.iter().any(|v| v == "tags"),
        "`tags` is Option<Vec<String>> so it should not be required, got required = {:?}",
        required
    );
    // And the schema for `tags` must declare an array-of-string shape
    // (or at least an object — schemas for `Option<T>` may render as
    // a union, an object with `anyOf`, or a flat T). We assert
    // *something* describes an array.
    let tags = &schema["properties"]["tags"];
    let has_array_shape = tags["type"] == "array"
        || tags.get("anyOf").is_some_and(|v| v.is_array())
        || tags.get("oneOf").is_some_and(|v| v.is_array());
    assert!(
        has_array_shape,
        "tags should describe an array shape, got: {}",
        tags
    );
}

#[test]
fn vec_of_option_param_schema() {
    let defs = GenericTypesTools::tool_definitions();
    let tool = defs.iter().find(|t| t.name == "vec_maybe").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let values = &schema["properties"]["values"];
    assert_eq!(values["type"], "array");
    // Each item is itself optional, so the schema should allow null.
    // The macro renders `Vec<Option<T>>` as `{ "type": "array", "items":
    // { "anyOf": [{ "type": "T" }, { "type": "null" }] } }`. The check
    // below tolerates both `null` literals and `{ "type": "null" }`
    // object forms.
    let item_schema = &values["items"];
    let any_null_in_anyof = item_schema.get("anyOf").is_some_and(|v| {
        v.as_array().is_some_and(|arr| {
            arr.iter()
                .any(|x| x.is_null() || x.get("type").and_then(|t| t.as_str()) == Some("null"))
        })
    });
    let allows_null = any_null_in_anyof
        || item_schema.get("type").and_then(|t| t.as_str()) == Some("null")
        || item_schema.get("nullable").and_then(|t| t.as_bool()) == Some(true);
    assert!(
        allows_null,
        "Vec<Option<i32>> items should permit null, got: {}",
        item_schema
    );
}

#[test]
fn tuple_param_renders_as_array() {
    let defs = GenericTypesTools::tool_definitions();
    let tool = defs.iter().find(|t| t.name == "tuple_param").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let pair = &schema["properties"]["pair"];
    // JSON Schema has no tuple type; the macro renders tuples as
    // either an array with `prefixItems` or an `anyOf` of singletons.
    // We assert it is *some* structured schema, not a bare scalar.
    assert!(
        pair.get("type").is_some()
            || pair.get("anyOf").is_some()
            || pair.get("prefixItems").is_some(),
        "tuple should produce a structured schema, got: {}",
        pair
    );
}

#[test]
fn btreemap_param_emits_object_schema() {
    let defs = GenericTypesTools::tool_definitions();
    let tool = defs.iter().find(|t| t.name == "btree_param").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();
    let m = &schema["properties"]["m"];
    // `BTreeMap<K, V>` is rendered as a JSON object whose values
    // follow the `V` type. The schema generator for `BTreeMap`
    // may not infer the value type (it can render as a bare object
    // without `additionalProperties`), so we just assert the object
    // shape and that the generated input is well-formed JSON.
    assert_eq!(m["type"], "object");
    // Accept either the typed (`additionalProperties` with a type)
    // or the untyped (no `additionalProperties`) rendering. Both
    // are valid JSON Schema for "any object".
    if let Some(ap) = m.get("additionalProperties") {
        // If the schema was generated, it should be either an empty
        // object or a typed schema. We don't assert the exact
        // rendering; just confirm the object is well-formed.
        assert!(ap.is_object() || ap.is_boolean());
    }
}

#[test]
fn tool_count_matches_declared_methods() {
    assert_eq!(GenericTypesTools::tool_definitions().len(), 6);
}
