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

## Cross-crate version assertion (T-024)

T-013 (intra-crate) and T-020 (intra-crate schema evolution)
cover version drift *inside* a single crate's tool set. T-024
extends that story to *cross-crate* drift: a consumer pinned
to `tokitai = "0.7"` and a third-party crate that bumped to
`tokitai = "0.8"` both compile, but the consumer's
`set_current_version` has no view of the third-party's
version. The agent sees a shape from one version and a call
site from the other.

T-024 closes the gap with two structural defenses:

1. **Compile-time check (consumer-side).** A downstream crate
   that wants to declare its expected `tokitai-core` version
   writes
   `const _: () = tokitai_core::assert_compatible_with("0.6");`
   in any `const` context. A mismatch fails to build with a
   `compile_error!` naming both versions and the docs.rs
   migration link. The match rule is canonical SemVer prefix
   match: `"0.6"` matches `0.6.0`, `0.6.1`, ...; `"0.6.1"`
   matches exactly; `"0"` matches any `0.x.y`. A `v` prefix is
   accepted transparently.

2. **Runtime check (server-side).** `tokitai-mcp-server`'s
   `serve()` reads `--require-tokitai=<prefix>` and
   `--allow-tokitai-mismatch` from `std::env::args`. The
   resolved `tokitai-core` version is baked into the binary
   at compile time via the build-script-emitted manifest
   `OUT_DIR/tokitai_manifest.rs` (sourced from `Cargo.lock`).
   On mismatch the server logs a `warn!` and refuses to start
   (returns `Err(ServerError::ServerStartupError(...))` so the
   surrounding `main` can exit with code 78 / `EX_CONFIG`).
   The override flag exists for documented emergency deploys
   and is itself logged at `warn!` level so the audit trail
   records the override.

### How T-013 / T-020 / T-024 fit together

| Layer | Mechanism | Surfaces drift at |
|-------|-----------|-------------------|
| **T-013** | `remove_in` / `replaced_by` per-method | Run time (a removed tool returns `ToolError::Removed`) |
| **T-020** | `since` / `until` per-method | Compile time (interval rules) + run time (gated dispatch) |
| **T-024** | `assert_compatible_with` + `serve()` | Compile time (consumer pins wrong version) + startup (server bin drift) |

Together the three layers cover the full version-drift story:
T-013/T-020 for *within-crate* schema evolution, T-024 for
*cross-crate* version drift at the dep boundary.

### Operator runbook

```bash
# Default: hard refusal on mismatch (exit 78 / EX_CONFIG).
tokitai-mcp-server --require-tokitai=0.6

# Documented emergency override. Logged at warn! level.
tokitai-mcp-server --require-tokitai=0.6 --allow-tokitai-mismatch

# NOTE: TOKITAI_VERSION_OVERRIDE was removed in M-0.9.1 as a
# security-hardening measure. Canary / staging deploys must
# use a release branch or patch Cargo.toml directly instead.
```

### Acceptance

- `tokitai_core::assert_compatible_with("0.6")` is a
  `pub const fn` and passes when called from a
  `const _ = ...` context with a matching / prefix-matching
  version.
- A consumer pinned to `tokitai = "0.7"` calling
  `assert_compatible_with("0.8")` fails to compile with a
  `compile_error!` naming both versions and the docs.rs
  migration link.
- A consumer pinned to `tokitai = "0.8.1"` calling
  `assert_compatible_with("0.8")` compiles cleanly (prefix
  match).
- `tokitai-mcp-server`'s `build.rs` writes
  `OUT_DIR/tokitai_manifest.rs` carrying the resolved
  `tokitai-core` version.
- `tokitai-mcp-server --require-tokitai=0.9.0` started
  against a binary compiled for `0.6.0` logs a `warn!` and
  refuses to bind.
- `tokitai-core/tests/assert_compatible_test.rs` and
  `tokitai-mcp-server/tests/version_assertion_test.rs`
  cover every branch (prefix / exact / `v`-prefix / malformed
  literal / override).

---

The following APIs are stable across the v0.6.x series:

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

The following APIs may change within the v0.6.x series:

| API | Description | Stability |
|-----|-------------|-----------|
| `MultiToolProvider` | Multi-tool provider | Experimental |
| `McpServerWithProvider<T>` | MCP server wrapper | Experimental |
| `ToolProvider::clone_definitions()` | Clone tool definitions | Experimental |

### Attribute syntax

The following attribute syntax is stable across the v0.4.x / v0.5.x / v0.6.x series:

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
- **At least six months of v0.6.x maintenance support**

---

## Version compatibility matrix

| Tokitai version | Minimum Rust | Compatibility notes |
|-----------------|--------------|---------------------|
| v0.6.x | 1.80+ | Current stable |
| v0.5.x | 1.80+ | Maintained, receives security fixes for six more months |
| v0.4.x | 1.80+ | Deprecated, please upgrade |
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
