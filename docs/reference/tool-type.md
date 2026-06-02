# `#[tool_type]`

> Attach a hand-written JSON Schema to a custom struct so that `#[tool]`
> methods can use it as a parameter or return type without falling back
> to the generic `object` schema.

## Syntax

```rust,ignore
#[tool_type(
    name = "TypeName",
    properties = "field_a: type, field_b: type",
    required = "field_a, field_b"
)]
pub struct TypeName {
    pub field_a: ...,
    pub field_b: ...,
}
```

`#[tool_type]` is a **block-level** attribute: it goes on a `struct`,
`enum`, or `type` alias definition. The attribute is processed at
macro-expansion time; the JSON Schema is stored in a process-global
`TYPE_SCHEMA_CACHE` and consulted by `#[tool]` whenever a parameter or
return type resolves to the named type.

## Arguments

| Argument | Type | Default | Description |
|---|---|---|---|
| `name` | `&str` | _required_ | Logical schema name; must match the type's Rust identifier or the lookup key. |
| `properties` | `&str` | _required_ | Comma-separated list of `field_name: type` pairs. |
| `required` | `&str` | `""` | Comma-separated list of required field names. |

The `properties` string accepts these shorthand type names:

| Shorthand | JSON Schema type |
|---|---|
| `string` | `{ "type": "string" }` |
| `integer` | `{ "type": "integer" }` |
| `number` | `{ "type": "number" }` |
| `boolean` | `{ "type": "boolean" }` |
| `array` | `{ "type": "array", "items": { … Any … } }` |
| `object` | `{ "type": "object", "properties": {} }` |

Anything else falls back to `{}` (`Any` in the schema).

## Examples

### Minimal

```rust,ignore
use tokitai::tool_type;

#[tool_type(
    name = "Point",
    properties = "x: number, y: number",
    required = "x, y"
)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
```

### Common usage

```rust,ignore
use tokitai::{tool, tool_type};
use serde::Deserialize;

#[tool_type(
    name = "Address",
    properties = "street: string, city: string, zip: string",
    required = "city"
)]
#[derive(Deserialize)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub zip: String,
}

#[tool_type(
    name = "User",
    properties = "id: integer, name: string, address: object, tags: array",
    required = "id, name"
)]
#[derive(Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub address: Option<Address>,
    pub tags: Vec<String>,
}

#[tool]
impl UserService {
    /// Create a new user record.
    pub async fn create(&self, user: User) -> u64 {
        user.id
    }
}
```

### Edge case

If the type is referenced by `#[tool]` and the cache has no schema for
it, the macro falls back to `{ "type": "object" }` (with no
`properties`). `#[tool_type]` is therefore mostly useful for the cases
where you want a richer schema than the auto-derived one (e.g. you
want to mark a field as `required` even though the Rust field is
`Option<T>`, or you want to give an enum a schema of `"string"` with
`enum` values).

```rust,ignore
use tokitai::tool_type;

#[tool_type(
    name = "Color",
    properties = "r: integer, g: integer, b: integer, a: integer",
    required = "r, g, b"
)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: Option<u8>,   // not required in the schema
}
```

## Generated code

`#[tool_type]` is **side-effect-only at proc-macro time**. The Rust
item the user wrote is emitted unchanged into the output; the
attribute's only effect is to insert a `(name, JsonSchema)` pair into a
process-global `BTreeMap<String, JsonSchema>` keyed by the struct's
identifier:

```rust,ignore
// What the macro produces (semantically):
//
// 1. The original struct definition, untouched:
//      pub struct Location { pub latitude: f64, pub longitude: f64 }
//
// 2. A side-effect: an entry in tokitai_core::TYPE_SCHEMA_CACHE
//      TYPE_SCHEMA_CACHE.insert(
//          "Location".to_string(),
//          JsonSchema::Object {
//              ty: "object".to_string(),
//              properties: {
//                  "latitude".to_string() => JsonSchema::number(None),
//                  "longitude".to_string() => JsonSchema::number(None),
//              },
//              required: vec!["latitude".to_string(), "longitude".to_string()],
//              // …other fields default…
//          },
//      );
```

The cache is consulted by the `#[tool]` macro's schema-generation
pipeline when a parameter or return type's `syn::Type` resolves to
`Location`. The generated JSON Schema is then embedded into the
`ToolDefinition::input_schema` string the same way any other
struct-derived schema would be.

Source: [`tokitai-macros/src/tool/mod.rs`](../../tokitai-macros/src/tool/mod.rs)
(`pub fn tool_type` and `impl ToolTypeAttrs::to_json_schema`).

## Interactions

- **With `#[tool]`**: `#[tool_type]` is the recommended way to attach
  a richer schema to a parameter or return type used by a `#[tool]`
  method. See [`tool.md`](tool.md).
- **With `config!`**: the cache entry is read at `__get_tool_definitions`
  time, so a `config!` override of the field-level descriptions on the
  type is applied normally. See [`config.md`](config.md).
- **With `#[wrap]` / `#[openapi]`**: works the same way — any method
  that uses the type as a parameter or return type will pick up the
  cached schema.
- **Crate scope**: the cache is a process-global `Mutex<BTreeMap>`,
  so two crates cannot both register the same `name` without one
  silently overwriting the other. The macro does not namespace by
  crate or by module.

## Errors

`#[tool_type]` is forgiving: an unparseable `properties` segment is
silently treated as `Any`, and a missing `name` becomes the empty
string. The macro does not currently produce any `compile_error!` or
`compile_warning!` of its own; failure to look up a cached schema at
`#[tool]`-expansion time is silent (it falls back to `object`).

If you want a strict mode (reject unknown shorthand types, reject
duplicate registrations), track [issue #42](https://github.com/silverenternal/tokitai/issues/42).

## See also

- Tutorial: [`docs/USAGE.md`](../USAGE.md) (`Custom type schemas`
  section).
- Architecture: [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) for
  the JSON-Schema pipeline.
- Rustdoc:
  [`tokitai-macros/src/lib.rs`](../../tokitai-macros/src/lib.rs)
  (`pub fn tool_type`).
- Example: [`examples/advanced_types.rs`](../../examples/advanced_types.rs).
- Example: [`examples/basic_usage.rs`](../../examples/basic_usage.rs)
  uses inline `#[tool_type]`-style structs as parameters.
