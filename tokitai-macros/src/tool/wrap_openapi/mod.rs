//! `#[openapi]` and `#[openapi_op]` proc-macro entry points.
//!
//! This module is the *only* surface that the `openapi` proc-macro
//! touches. It is split into three parts:
//!
//! - [`parser`] deserialises the OpenAPI 3 spec at proc-macro compile
//!   time.
//! - [`extract`] reads `#[openapi(...)]` and `#[openapi_op(...)]`
//!   attribute arguments.
//! - [`codegen`] emits the full `impl` block plus the
//!   `ToolProvider`/`ToolCaller` impls and the spec lookup static.
//!
//! The two entry points exposed to [`crate::lib`] are [`expand`]
//! (impl-block level) and [`expand_op`] (per-method).

use proc_macro2::TokenStream as TokenStream2;
use syn::{parse2, ItemImpl};

pub(crate) mod codegen;
pub(crate) mod extract;
pub(crate) mod parser;
pub(crate) mod spec_static;

/// Compile-time `#[openapi]` expansion.
///
/// Reads the spec file referenced by `args`, validates every
/// `#[openapi_op(operation_id = "...")]` method on `input`, and
/// splices a fully-formed `ToolProvider`/`ToolCaller` impl into the
/// output.
pub fn expand(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let parsed_args = match parse2::<extract::OpenApiArgs>(args) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };

    let spec_path_raw = match parsed_args.spec.as_deref() {
        Some(s) => s,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "`#[openapi]` requires `spec = \"...\"`",
            )
            .to_compile_error();
        }
    };

    // Resolve the spec path. Relative paths are resolved against the
    // source file containing the `#[openapi]` attribute (so the
    // user can drop `openapi.json` next to their `.rs` file and
    // just write `spec = "openapi.json"`). Absolute paths are
    // taken as-is.
    let resolved_spec_path = resolve_spec_path(spec_path_raw, &input);
    let spec_path = resolved_spec_path
        .as_deref()
        .unwrap_or(spec_path_raw);

    // Read the spec off disk at proc-macro time. If the file is
    // missing or unreadable, surface a friendly error so the user
    // sees actionable guidance instead of a raw `Os` error.
    let spec_text = match std::fs::read_to_string(spec_path) {
        Ok(t) => t,
        Err(e) => {
            // Translate the most common case (NotFound) into a
            // hand-written message that points the user at the
            // exact fix. Other errors fall through to a generic
            // `could not read` diagnostic.
            let detail = if e.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "the OpenAPI spec file `{}` was not found relative to the source file. \
                     Ensure it exists in the same directory as your Rust source file \
                     (the macro resolves relative paths the same way `include_str!` does). \
                     Underlying error: {}",
                    spec_path, e
                )
            } else {
                format!(
                    "`#[openapi]` could not read spec file `{}`: {}",
                    spec_path, e
                )
            };
            return syn::Error::new(proc_macro2::Span::call_site(), detail).to_compile_error();
        }
    };

    let spec = match parser::OpenApiSpec::from_str(&spec_text) {
        Ok(s) => s,
        Err(e) => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "`#[openapi]` could not parse spec `{}` as valid OpenAPI 3 JSON. \
                     Verify that the file is well-formed JSON (you can test with `jq . {}`) \
                     and conforms to the OpenAPI 3.0 or 3.1 schema. Underlying error: {}",
                    spec_path, spec_path, e
                ),
            )
            .to_compile_error();
        }
    };

    // Reject specs with no `paths` block. An OpenAPI document
    // without paths is, by definition, empty — proceeding would
    // just emit a no-op `impl ToolProvider` block, which is a
    // confusing failure mode.
    if spec.paths.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "`#[openapi]` spec `{}` declares no `paths`. An OpenAPI document without \
                 operations cannot be exposed as AI-callable tools. Add at least one path \
                 to the spec, or remove the `#[openapi]` attribute.",
                spec_path
            ),
        )
        .to_compile_error();
    }

    let impl_item = match parse2::<ItemImpl>(input) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    // Emit the resolved (absolute) path so the consumer's
    // `include_str!` macro can locate the spec at its compile
    // time. This keeps a single source of truth for the file's
    // location: the proc-macro reads it for code generation, the
    // consumer's compiler embeds it for the static.
    let emit_path = resolved_spec_path.as_deref().unwrap_or(spec_path_raw);
    codegen::expand_impl(&impl_item, &spec, emit_path)
}

/// Resolve a spec path string. If it's already absolute, return
/// it as-is. Otherwise, try to resolve it relative to the source
/// file containing the `#[openapi]` attribute. The proc-macro's
/// `Span` is the only handle we have on the consumer's source
/// file location at this stage.
fn resolve_spec_path(spec_path: &str, input: &TokenStream2) -> Option<String> {
    let p = std::path::Path::new(spec_path);
    if p.is_absolute() {
        return Some(spec_path.to_string());
    }

    // Best-effort: look at the first token's span. If it points to
    // a real file on disk, use its parent directory as the base.
    // We try `unwrap()` first to get the `proc_macro::Span`, then
    // `local_file()`. If either step fails (e.g. when the proc-
    // macro is invoked by a non-cargo driver), we fall back to
    // leaving the spec path unchanged.
    let first_token = input.clone().into_iter().next()?;
    let first_span = first_token.span();
    let source_path: std::path::PathBuf = match first_span.unwrap().local_file() {
        Some(p) => p,
        None => return None,
    };

    if !source_path.is_absolute() {
        return None;
    }

    let base = source_path.parent()?;
    let joined = base.join(p);
    Some(joined.to_string_lossy().into_owned())
}

/// Per-method `#[openapi_op]` expansion.
///
/// Acts as a transparent passthrough that records the `operation_id`
/// for the impl-block-level [`expand`] to pick up. The actual
/// metadata is gathered when the impl block is processed, not here.
pub fn expand_op(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    // Validate the attribute parses cleanly so user-facing typos
    // surface at the method's call site rather than the impl block.
    if let Err(e) = parse2::<extract::OpenApiOpArgs>(args) {
        return e.to_compile_error();
    }
    // Hand the method back to the compiler untouched — the impl
    // block re-walks it.
    input
}
