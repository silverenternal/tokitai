# API Stability Commitment

This document records Tokitai's API stability policy and version-compatibility guarantees.

---

## Versioning policy

Tokitai follows [Semantic Versioning 2.0](https://semver.org/spec/v2.0.0.html):

- **Major version**: breaking changes
- **Minor version**: new features, backward-compatible
- **Patch version**: bug fixes, backward-compatible

---

## Tool versioning (T-013)

`#[tool]` is the first-class surface for managing the lifecycle of
a tool. Each `#[tool]` attribute can carry four fields that work
together to encode the deprecation schedule:

| Field | Type | Purpose |
|-------|------|---------|
| `version` | string | The version of the tool that introduced this method. Surface as `ToolDefinition.version`. |
| `deprecated_since` | string | Version since which the method has been deprecated. Surface as `ToolDefinition.deprecated_since`. |
| `remove_in` | string | Version at or after which the macro refuses to call the method. Surface as `ToolDefinition.remove_in`. |
| `replaced_by` | string | Name of the tool that succeeds this one. Surface as `ToolDefinition.replaced_by`. |

### Lifecycle stages

| Stage | Marker | Runtime behaviour |
|-------|--------|-------------------|
| **Current** | No `deprecated_*` attribute | Calls succeed; the LLM client sees no deprecation marker. |
| **Deprecated** | `deprecated = true` (or any of the lifecycle fields) | Calls succeed; the provider envelope (`to_openai_function` / `to_anthropic_tool` / `to_mcp_tool`) carries a deprecation marker (MCP `_meta.deprecated` + `deprecatedSince`, OpenAI / Anthropic description suffix) the LLM can read. |
| **Removed** | `remove_in <= current_version` | Calls return `ToolError::Removed` (new variant, `kind = ToolErrorKind::Removed`). The error message names the removed tool, the `remove_in` boundary, and — when one is configured — the `replaced_by` successor so the caller can retry with the new name. |

### Activation

The dispatcher only gates on `remove_in` once the program has
called `tokitai::set_current_version("X.Y.Z")`. Without a current
version the call path is open and `remove_in` is informational.
This matches `tokitai_core::set_async_executor` ergonomics: set
once at startup, ignore if unknown.

```rust
use tokitai::set_current_version;

fn main() {
    set_current_version(env!("CARGO_PKG_VERSION"));
    // ... runtime proceeds; tools with `remove_in <= current` now error.
}
```

### `replaced_by` redirect

When the caller names a tool that is not in the active match
arms (e.g. it was deleted from the impl block) but a `replaced_by`
link is still registered, the dispatcher's fallback arm
re-invokes `call_tool` with the successor. If the successor also
does not exist the call returns `NotFound` after one redirect hop
— the dispatcher does not loop.

### SemVer

`remove_in` and the program-wide current version are compared
with the canonical SemVer comparison `(major, minor, patch)`. A
typo in either string fails open (no gating) so a malformed
version never silently removes a live tool. Pre-release and build
metadata (`-alpha.1`, `+build.42`) are ignored for ordering.

To enforce strict SemVer at the macro level, add a follow-up
`#[tool(version_policy = "semver")]` attribute (T-013 design
question Q-5, deferred to a later release).

---

## Schema evolution (T-020)

T-013's `replaced_by` covers the rename case. The remaining
pain is the additive-vs-breaking distinction across versions:
adding an optional field is non-breaking across releases, but
removing a field or changing its type is breaking, and the
dispatcher has no way to tell. T-020 introduces two
per-method attributes that declare the schema-evolution
interval of each `#[tool]` method:

| Field | Type | Purpose |
|-------|------|---------|
| `since` | string | Lower bound (inclusive) of the schema's evolution interval. The dispatcher hides the method when `current_version() < since`. |
| `until` | string | Upper bound (exclusive) of the interval. The dispatcher hides the method when `current_version() >= until`. |

When combined with `tokitai_core::set_current_version(...)`, the
dispatcher serves the method whose interval contains the current
version. Multiple methods in the same `impl` block can declare
non-overlapping intervals so each generation of the schema
replaces its predecessor without forcing every caller to migrate
in lockstep:

```rust
#[tool(version_policy = "semver")]
impl UserApi {
    #[tool(since = "1.0.0", until = "2.0.0")]
    pub fn query_v1(&self, sql: String) -> Value { /* ... */ }

    #[tool(since = "2.0.0")]
    pub fn query_v2(&self, sql: String, params: Option<Value>) -> Value { /* ... */ }
}
```

At `current_version() = "1.5.0"` only `query_v1` is exposed to the
LLM. At `current_version() = "2.0.0"` only `query_v2` is
exposed. The half-open `[since, until)` interval makes the
boundary deterministic and easy to reason about (no off-by-one
between consecutive versions).

### Compile-time interval checks

When `version_policy = "semver"` is set on the impl block, the
macro enforces three rules at compile time:

1. **Strict parse**: every `since = "..."` / `until = "..."`
   literal must parse as SemVer (with an optional `v` prefix).
   CalVer (`2026.06`) and commit-SHA strings are rejected
   because they cannot be compared with SemVer rules.
2. **Non-empty interval**: `since` must be strictly less than
   `until`. A method whose interval is empty would never be
   served by the dispatcher.
3. **No overlap**: intervals across the same impl must tile the
   version line without overlap. Two methods whose intervals
   overlap would both be candidates for some
   `current_version`, and the dispatcher picks the first
   match in declaration order — a recipe for stale schemas.

Loose strings (CalVer, commit SHA) are accepted when the impl
does NOT opt into `version_policy = "semver"`. The macro skips
the strict parse and overlap checks, and the runtime uses
lexicographic ordering via `tokitai_core::parse_semver`'s
fallback path. This is the recommended escape hatch for
projects whose version policy is not SemVer.

### Activation

The version filter only runs when the program has called
`tokitai_core::set_current_version(...)`. Without a registered
version every method is served (the macro's fast path returns
the full static slice unchanged), so existing consumers see no
behaviour change. The cached filtered view is keyed by the
version string itself, so changing the version triggers
exactly one fresh allocation; repeated calls with the same
version hit the cache.

### Acceptance

- The `#[tool(since = "1.0", until = "2.0")]` and
  `#[tool(since = "2.0")]` attributes on two methods in the
  same impl block compile cleanly.
- `set_current_version("1.5")` exposes only the 1.0 method;
  `set_current_version("2.0")` exposes only the 2.0 method.
- An empty interval (`since == until`) is a compile error
  anchored at the offending method's span.
- A CalVer string under `version_policy = "semver"` is a
  compile error recommending the user drop the policy.
- `tokitai/tests/schema_evolution_test.rs` covers the cases
  above, including additive changes (new optional field) and
  the backwards-compatible default (no current version ->
  every method is served).

---

The following APIs are stable across the v0.5.x series:

### Stable APIs

| API | Description | Stability |
|-----|-------------|-----------|
| `#[tool]` macro | Core procedural macro | Stable |
| `ToolProvider::tool_definitions()` | Get tool definitions | Stable |
| `ToolProvider::call_tool()` | Call a tool | Stable |
| `ToolDefinition` | Tool-definition struct | Stable |
| `ToolError` | Tool error type | Stable |
| `SchemaGenConfig` | Schema-generation configuration | Stable |

### Experimental APIs

The following APIs may change within the v0.5.x series:

| API | Description | Stability |
|-----|-------------|-----------|
| `MultiToolProvider` | Multi-tool provider | Experimental |
| `McpServerWithProvider<T>` | MCP server wrapper | Experimental |
| `ToolProvider::clone_definitions()` | Clone tool definitions | Experimental |

### Attribute syntax

The following attribute syntax is stable across the v0.4.x / v0.5.x series:

```rust
// Method-level attributes
#[tool(name = "custom_name", desc = "Custom description")]
#[tool(skip)]
#[tool(deprecated = true, replaced_by = "new_method")]
#[tool(alias = ["alias1", "alias2"])]
#[tool(tags = ["tag1", "tag2"])]
#[tool(visible = false)]

// Parameter-level attributes (declared on the method)
#[tool(min_length_param = 1, max_length_param = 100)]
#[tool(min_param = 0, max_param = 150)]
#[tool(example_param = "example_value")]
#[tool(default_param = null)]
#[tool(validate_param = "value > 0")]

// Doc-comment syntax
/// @param name parameter description
/// @validate name !value.is_empty()
/// @required name
```

---

## v1.0.0 plan

### Release criteria

Before v1.0.0 ships, the following must be true:

- [ ] All stable APIs have been battle-tested in production for at least three months
- [ ] No outstanding P0/P1 bugs
- [ ] Complete documentation and migration guide
- [ ] Community feedback has been collected and addressed

### Guarantees

After v1.0.0 ships:

- **All public APIs remain backward-compatible throughout the v1.x series**
- **Breaking changes are deferred to v2.0.0**
- **At least six months of v0.5.x maintenance support**

---

## Version compatibility matrix

| Tokitai version | Minimum Rust | Compatibility notes |
|-----------------|--------------|---------------------|
| v0.5.x | 1.80+ | Current stable |
| v0.4.x | 1.80+ | Maintained, receives security fixes for six more months |
| v0.3.x | 1.80+ | Deprecated, please upgrade |
| v1.0.0 (planned) | 1.80+ | Long-term support |

---

## Breaking-change policy

When a breaking change is unavoidable:

1. **Advance notice**: announce in `CHANGELOG.md` and on the GitHub release page
2. **Migration guide**: provide step-by-step instructions and code samples
3. **Transition period**: at least one minor version of overlap
4. **Automated migration**: ship a codemod whenever feasible

### Example: v0.3 to v0.4 migration

**Change**: `TOOL_DEFINITIONS` constant becomes the `tool_definitions()` method.

**Migration steps**:

```rust
// Old code (v0.3)
let tools = Calculator::TOOL_DEFINITIONS;

// New code (v0.4)
let tools = Calculator::tool_definitions();
```

---

## Feedback channels

For API questions or suggestions:

- **GitHub issues**: https://github.com/silverenternal/tokitai/issues
- **Discussions**: https://github.com/silverenternal/tokitai/discussions

---

*Last updated: 2026-03-10*
