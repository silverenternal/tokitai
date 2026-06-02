# API Stability Commitment

This document records Tokitai's API stability policy and version-compatibility guarantees.

---

## Versioning policy

Tokitai follows [Semantic Versioning 2.0](https://semver.org/spec/v2.0.0.html):

- **Major version**: breaking changes
- **Minor version**: new features, backward-compatible
- **Patch version**: bug fixes, backward-compatible

---

## v0.5.x series - stable API

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
