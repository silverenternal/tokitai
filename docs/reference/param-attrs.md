# Per-parameter attributes

> 16 small proc-macro attributes that hook into a `#[tool]` method's
> parameter list to attach JSON-Schema constraints, defaults,
> examples, descriptions, aliases, hidden flags, and deprecation
> hints. They are no-op at expansion time — `#[tool]` re-parses them
> from the surrounding `#[tool(...)]` block.

## Syntax

There are three ways to attach per-parameter metadata to a `#[tool]`
method:

1. **Per-parameter attribute** (canonical): one of the 16
   `#[tool_*]` attributes on the parameter itself.

   ```rust,ignore
   #[tool]
   impl MyService {
       pub fn create(&self,
           #[tool_min = 0]            age: i32,
           #[tool_pattern = "^.+@.+$"] email: String,
       ) -> Result<(), String> { /* … */ }
   }
   ```

2. **Method-level key in `#[tool(...)]`**: prefix the parameter name
   with the attribute name.

   ```rust,ignore
   #[tool(min_age = 0, pattern_email = "^.+@.+$")]
   impl MyService {
       pub fn create(&self, age: i32, email: String) -> Result<(), String> { /* … */ }
   }
   ```

3. **Bundled `#[param_tool(...)]`** (one attribute, multiple keys):

   ```rust,ignore
   pub fn create(&self,
       #[param_tool(min = 0, max = 150, desc = "User age")]
       age: i32,
       #[param_tool(pattern = "^.+@.+$", example = "a@b.com")]
       email: String,
   ) -> Result<(), String> { /* … */ }
   ```

All three forms produce the same generated schema; the per-parameter
attribute form is most useful when you have one or two constraints,
the method-level form is most useful when the same constraint
applies to many parameters, and the `#[param_tool]` bundled form is
most useful when a parameter has several constraints.

## Arguments

The 16 per-parameter attributes fall into six groups.

### Numeric bounds

| Attribute | Type | JSON Schema | Description |
|---|---|---|---|
| `#[tool_min = N]` | `f64` | `minimum` | Value `>= N`. |
| `#[tool_max = N]` | `f64` | `maximum` | Value `<= N`. |
| `#[tool_multiple_of = N]` | `f64` | `multipleOf` | Value is a multiple of `N`. |

### String bounds

| Attribute | Type | JSON Schema | Description |
|---|---|---|---|
| `#[tool_min_length = N]` | `usize` | `minLength` | Length `>= N`. |
| `#[tool_max_length = N]` | `usize` | `maxLength` | Length `<= N`. |
| `#[tool_pattern = "regex"]` | `&str` | `pattern` | Must match the regex. |
| `#[tool_alias = ["n2", …]]` | `[&str]` | _(meta)_ | Aliases for the schema name. |

### Array bounds

| Attribute | Type | JSON Schema | Description |
|---|---|---|---|
| `#[tool_min_items = N]` | `usize` | `minItems` | Length `>= N`. |
| `#[tool_max_items = N]` | `usize` | `maxItems` | Length `<= N`. |

### Enumerations

| Attribute | Type | JSON Schema | Description |
|---|---|---|---|
| `#[tool_one_of = ["a", "b"]]` | `[&str]` | `enum` | One of the listed strings. |
| `#[tool_enum_values = [...]]` | `[JSON value]` | `enum` | One of the listed JSON values. |

### Documentation / metadata

| Attribute | Type | JSON Schema | Description |
|---|---|---|---|
| `#[tool_desc = "…"]` | `&str` | `description` | Parameter description. |
| `#[tool_example = …]` | JSON | `examples` | Example value. |
| `#[tool_default = …]` | JSON | `default` | Default value. |
| `#[tool_required]` | _flag_ | (added to `required`) | Mark `Option<T>` as required. |
| `#[tool_hidden]` | _flag_ | _(removed)_ | Hide from the schema. |
| `#[tool_deprecated]` | _flag_ | `deprecated` | Mark as deprecated. |
| `#[tool_validate = "expr"]` | `&str` | _(runtime)_ | Validation; `value` = the param. |
| `#[tool_transform = "expr"]` | `&str` | _(runtime)_ | Transformation; result is passed to the method. |

## Method-level key form

Inside a `#[tool(...)]` block, every per-parameter attribute is
also accepted as a `key_<param> = value` pair. The accepted
prefixes follow the parser in
[`tokitai-macros/src/tool/attrs/method.rs`](../../tokitai-macros/src/tool/attrs/method.rs):
`min_<param>`, `max_<param>`, `min_length_<param>`,
`max_length_<param>`, `min_items_<param>`, `max_items_<param>`,
`multiple_of_<param>`, `pattern_<param>`, `one_of_<param>`,
`enum_values_<param>`, `default_<param>`, `example_<param>`,
`validate_msg_<param>` (and the `_zh` / `_en` locale overrides).

## Bundled form: `param_tool`

`#[param_tool(...)]` is a thin wrapper that accepts the same
constraint keys (`validate`, `transform`, `desc`, `default`,
`example`, `required`) in a single attribute group. It is
functionally identical to stacking the per-parameter attributes on
the same parameter.

```rust,ignore
#[param_tool(
    desc = "User age in years",
    min = 0,
    max = 150,
    example = 30,
    required,
)]
age: i32,
```

> **Note on the macro name**: `param_tool` is the bundled form of
> the per-parameter attributes. The original spec for this
> reference split it out as a standalone item, but it is documented
> here under its parent concept. There is no separate
> `param_tool.md`.

## Examples

### Minimal

```rust,ignore
use tokitai::tool;

#[tool]
impl User {
    /// Create a user.
    pub fn create(&self,
        #[tool_min_length = 1]    // name must be non-empty
        #[tool_max_length = 50]   // and at most 50 chars
        name: String,
    ) -> String { name }
}
```

### Common usage

```rust,ignore
use tokitai::tool;

#[tool]
impl Account {
    /// Open a new account.
    #[tool(
        desc = "Open a new account with the given email and age.",
        tags = ["account", "write"],
        min_age = 0,
        max_age = 150,
        pattern_email = r"^[^@\s]+@[^@\s]+\.[^@\s]+$",
        example_email = "alice@example.com",
        one_of_currency = ["USD", "EUR", "JPY"],
        default_currency = "USD",
    )]
    pub fn open(&self,
        name: String,
        age: i32,
        email: String,
        currency: Option<String>,
    ) -> String {
        format!("{name} ({age}, {email}, {})", currency.unwrap_or_else(|| "USD".into()))
    }
}
```

### Edge case

`#[tool_required]` on an `Option<T>` parameter — useful when the
parameter is structurally optional in Rust but business-logic
requires it. The literal `value` in `validate` / `transform`
expressions is replaced with the deserialised parameter variable
name at codegen time, so the expression runs in a context where
`value: T` is the just-parsed parameter.

```rust,ignore
use tokitai::tool;

#[tool]
impl Signup {
    pub fn create(&self,
        #[tool_required]                       // schema marks `email` as required
        email: Option<String>,
        #[tool_default = 18]                   // schema says default = 18
        age: Option<i32>,
        #[tool_validate = "value.contains('@')"]
        #[tool_transform = "value.trim().to_lowercase()"]
        raw_email: String,
    ) -> String {
        format!("{} {}", email.unwrap_or_default(), age.unwrap_or(18))
    }
}
```

## Generated code

The 16 `#[tool_*]` attributes are no-op at expansion time — they
forward the parameter through unchanged. `#[tool]` re-parses them
from the parameter's `syn::Attribute` list, then uses the parsed
values to build the `ParamInfo` for the schema generator and the
runtime validator.

What the generated `__call_<name>` wrapper does for a parameter
with `#[tool_min = 0]`, `#[tool_max = 150]`: parse the parameter
as the declared Rust type via `serde_json::from_value` (allowing
`Option<T>` to be missing, substituting the default if configured),
run each constraint in order (`one_of` → `pattern` → `min` → `max`
→ `min_length` → `max_length` → `min_items` → `max_items` →
`multiple_of` → `validate`), then run the `transform` expression
(if any) and pass the result to the user method.

A representative excerpt from
[`tokitai-macros/src/tool/codegen/wrappers.rs`](../../tokitai-macros/src/tool/codegen/wrappers.rs):

```rust,ignore
// min / max for numeric parameters
if #param_name < #min_value {
    return Err(::tokitai::ToolError::validation_error(
        format!("parameter '{}' value {} is below the minimum {}", …)
    ));
}
if #param_name > #max_value {
    return Err(::tokitai::ToolError::validation_error(
        format!("parameter '{}' value {} is above the maximum {}", …)
    ));
}

// min_length / max_length for strings
if #param_name.len() < #min_length {
    return Err(::tokitai::ToolError::validation_error(
        format!("parameter '{}' length {} is below the minimum length {}", …)
    ));
}

// pattern
if !#pattern.is_match(&#param_name) {
    return Err(::tokitai::ToolError::validation_error(
        format!("parameter '{}' value '{}' does not contain required pattern: {}", …)
    ));
}

// one_of
if !["a", "b", "c"].contains(&#param_name.as_str()) {
    return Err(::tokitai::ToolError::validation_error(
        format!("parameter '{}' value '{}' is not in the allowed set: {}", …)));
}

// multiple_of / user-supplied validate
if #param_name % #multiple_of_value != 0.0 { /* … */ }
if !(#user_validate_expr) {
    return Err(::tokitai::ToolError::validation_error(#user_msg_or_default));
}
```

The error message template is localisable: if the user supplied
`validate_msg_zh_<param>`, the wrapper checks
`std::env::var("LANG")` and `std::env::var("LC_ALL")` and returns the
Chinese string when either starts with `"zh"`. Otherwise the
English `validate_msg_en_<param>` (if given) is used, or the
default English template is used as a fallback.

The schema fragments (`"minimum": 0`, `"pattern": "..."`, etc.)
are emitted by
[`tokitai-macros/src/tool/schema/gen.rs`](../../tokitai-macros/src/tool/schema/gen.rs)
into the `ToolDefinition::input_schema` JSON string.

## Interactions

- **With `#[tool]`**: required. None of the per-parameter attributes
  have any effect outside a `#[tool]` method.
- **With `#[wrap]` / `#[openapi]` / `#[delegate]`**: all three
  reuse the `#[tool]` codegen pipeline, so the per-parameter
  attributes work the same way. See
  [`wrap.md`](wrap.md), [`openapi.md`](openapi.md),
  [`delegate.md`](delegate.md).
- **With `config!`**: `config!` can override parameter descriptions
  and examples at runtime; the per-parameter attributes are
  compile-time. The runtime override wins (last-write-wins order
  is: doc comment → `#[tool_*]` → `config!`).
- **With `#[tool_hidden]`**: the parameter is still parsed at
  runtime but is omitted from `input_schema`. Useful for
  credentials or context that should not be exposed to the LLM.

## Errors

The per-parameter attributes are parsed by `#[tool]`; failures
surface as `syn::Error` from the `#[tool]` expansion, with messages
like `"expected string literal"` or `"expected integer literal"`.
The most common failure modes:

| Trigger | Message |
|---|---|
| `min` / `max` value is not a numeric literal | syn parse error pointing at the value |
| `pattern` value is not a string literal | syn parse error |
| `one_of` list contains a non-string-literal entry | syn parse error |
| `enum_values` list contains a non-literal entry | the entry is rendered via `quote::ToTokens` and may produce surprising output — prefer JSON literals |
| `validate` / `transform` expression does not parse as a Rust `Expr` | `[tokitai] warning: failed to parse validation expression: <code> - <err>` (under non-`cfg(test)` builds) |
| `default` value is a complex expression | the literal is parsed via `parse_json_value`; non-JSON literals (`Some(1)`, `vec![1, 2]`) fall through to `quote::ToTokens` and may produce surprising schema output |

The macro produces no warnings of its own for missing per-parameter
attributes; the only warnings (W001 / W002 / W003) come from the
parent `#[tool]` block.

## See also

- Tutorial: [`docs/USAGE.md`](../USAGE.md) (`Parameter constraints`
  section).
- Architecture: [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) (the
  per-parameter schema pipeline).
- Rustdoc:
  [`tokitai-macros/src/lib.rs`](../../tokitai-macros/src/lib.rs)
  (`pub fn param_tool` and the 15 `pub fn tool_*` no-op wrappers).
- Examples:
  [`examples/param_attrs.rs`](../../examples/param_attrs.rs)
  (three description styles),
  [`examples/validate_transform_alias.rs`](../../examples/validate_transform_alias.rs)
  (`validate`, `transform`, `alias`),
  [`examples/advanced_types.rs`](../../examples/advanced_types.rs)
  (`min_length`, `max_length`, `pattern`, `one_of`).
