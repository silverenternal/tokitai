# ADR-0006: Spec path resolution for `#[openapi]` uses `Span::local_file()`

- **Status:** Accepted
- **Date:** 2026-06-02
- **Authors:** Tokitai maintainers

## Context

The `#[openapi(spec = "openai_chat.json")]` proc-macro reads a
spec file from disk at compile time. The path in the attribute
is a **string** the user wrote, and the macro must turn it into
a **filesystem path** the compiler can `read_to_string`.

Three resolution strategies were on the table:

1. **Relative to the source file** — match `include_str!` semantics
   so users can place the spec next to their Rust file.
2. **Relative to `CARGO_MANIFEST_DIR`** — resolve against the
   crate root.
3. **Always absolute** — force the user to write
   `/abs/path/to/spec.json` or
   `env!("CARGO_MANIFEST_DIR") + "/openai.json"`.

The first option is the most ergonomic: the user can put the
spec next to their `lib.rs` and write
`#[openapi(spec = "openai_chat.json")]` without thinking about
it. The second option forces a layout (specs in `src/openapi/`
are common, which would not resolve). The third option is
hostile to monorepos where the spec is checked in next to the
API client module.

The challenge with option 1 is that the proc-macro host has no
direct way to ask "what is the source file this attribute is
attached to?" The only handle we have is the `Span` of the
attribute's first token. `Span::local_file()` returns the
absolute path of the file the token came from, **if** the file
is on disk and the proc-macro server has access to it.

## Decision

Relative paths in `#[openapi(spec = "openai_chat.json")]` are
resolved against the source file's parent directory via
`proc_macro2::Span::local_file()`. Absolute paths are taken as-is.

The implementation
([`tokitai-macros/src/tool/wrap_openapi/mod.rs`](../../tokitai-macros/src/tool/wrap_openapi/mod.rs))
is:

```rust
fn resolve_spec_path(spec_path: &str, input: &TokenStream2) -> Option<String> {
    let p = std::path::Path::new(spec_path);
    if p.is_absolute() {
        return Some(spec_path.to_string());
    }
    let first_token = input.clone().into_iter().next()?;
    let source_path = first_token.span().unwrap().local_file()?;
    if !source_path.is_absolute() { return None; }
    let base = source_path.parent()?;
    Some(base.join(p).to_string_lossy().into_owned())
}
```

If `local_file()` returns `None` (e.g. when the source is being
fed over stdin, or the macro is being run in a non-cargo test
harness), the path is left unchanged. The user in that situation
is responsible for using an absolute path.

## Consequences

**Easier:**

- Users can write `#[openapi(spec = "openai_chat.json")]` and
  drop the spec next to their Rust file. No `env!` calls, no
  `CARGO_MANIFEST_DIR` juggling, no absolute paths.
- The semantics match `include_str!` exactly, so users coming
  from `include_str!`-style macros already know the rules.
- Absolute paths are honoured as-is, which is what monorepo
  users need (the spec lives at the workspace root).

**Harder:**

- When the source file is not on disk — most commonly when the
  proc-macro is invoked from a test that synthesises a source
  file in memory — `local_file()` returns `None` and the spec
  path is treated as a workspace-relative path. Users in that
  situation must use an absolute path. The `trybuild` UI test
  suite for `#[tool]` exercises exactly this case; we use
  absolute paths in those tests.
- The macro depends on `proc_macro2::Span::local_file()`
  returning a valid path. Older proc-macro servers (some custom
  build systems) may return `None` here. Acceptable because the
  macro is useless without a real file on disk.
- The behaviour is **best-effort**. There is no compile-time
  error if the resolution fails; the macro falls through to the
  original string and lets `std::fs::read_to_string` surface the
  actual `NotFound`. A typo shows up as
  `openai_chat.jso: not found` rather than
  `spec not found relative to <source>`.

## Alternatives considered

- **`CARGO_MANIFEST_DIR`** — rejected. Forces the spec to live
  under the crate root. Users with `src/openapi/spec.json` or
  `specs/openai.json` would have to either move the spec or
  write a long relative path.
- **`env!("CARGO_MANIFEST_DIR")` at runtime** — rejected. Makes
  the spec path fragile to monorepo layouts (a workspace member
  depending on a spec in a sibling crate would have to walk up
  with `..`). The proc-macro-time resolution via
  `Span::local_file()` is the only way to get a path anchored
  to **the file containing the attribute**, not the build
  manifest.
- **A new `#[openapi(spec_inline = "...")]` that takes a string
  literal** — rejected. The point of the macro is to read a
  file from disk. If the user wants an inline spec, they can
  use `include_str!` to pull a checked-in spec into a
  `&'static str`. We do not think this is a common case.
