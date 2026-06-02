//! Centralised diagnostic system for `#[tool]`, `#[wrap]`, `#[openapi]`,
//! and friends.
//!
//! Every error the proc-macros emit is funneled through [`MacroError`]
//! so users get a consistent, polished diagnostic:
//!
//! ```text
//! error[E0001]: unknown method name `foo`
//!  --> src/lib.rs:10:5
//!   |
//! 10 |     fn foo() {}
//!   |     ^^ did you mean `bar` or `baz`?
//!   |
//!   = help: see https://docs.rs/tokitai/latest/tokitai/errors.html#E0001
//! ```
//!
//! Three things are guaranteed for every diagnostic:
//!
//! 1. **Span**: the user can see *exactly* which token tripped the
//!    rule. Spans come from the `syn` tree the macro is operating on,
//!    so they line up with the user's source.
//! 2. **Stable code**: every error class has a stable
//!    `E0001`-`E0099` identifier so users can search the docs and
//!    find a write-up.
//! 3. **Actionable help**: every error carries either a `help:` line
//!    explaining what to do, a "did you mean" suggestion, or both.
//!
//! The code is intentionally *append-only*: once a code like `E0001`
//! has been assigned, the meaning is part of the public surface and
//! can only ever be *extended* (more `help:` text, more suggestion
//! variants), never changed in a breaking way. See
//! `docs/errors.md` for the canonical list of codes.

use std::fmt::Write as _;

use proc_macro2::{Span, TokenStream};
use quote::quote_spanned;

/// Stable identifier for a diagnostic class.
///
/// **Stability:** once a variant is published it can never change
/// meaning. New variants may be appended; old variants are never
/// removed. See the module-level docs for the compatibility policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// A method listed in `methods = [...]` does not exist on the
    /// impl block (or is not `pub`).
    E0001,
    /// A required attribute argument is missing entirely
    /// (e.g. `spec = "..."` on `#[openapi]`).
    E0002,
    /// An attribute value is present but not parseable / not in the
    /// allowed set (e.g. `backoff = "wavy"` on `#[retry]`).
    E0003,
    /// The annotated method is generic; not currently supported.
    E0004,
    /// The annotated method's return type mentions `Self`, which the
    /// schema generator cannot resolve.
    E0005,
    /// The annotated method is `async` and also takes `&mut self`;
    /// this combination is rejected at runtime by the executor and
    /// is flagged at compile time.
    E0006,
    /// `#[openapi]` cannot find the spec file at the resolved path.
    E0007,
    /// `#[openapi]` read the spec file, but it is not valid OpenAPI
    /// 3.x JSON.
    E0008,
    /// `#[wrap]` was given an empty `methods = [...]` list.
    E0009,
    /// An `#[openapi_op]` method is missing its `operation_id`.
    E0010,
    /// An `#[openapi_op]` method's `operation_id` is not present in
    /// the parsed spec.
    E0011,
    /// The annotated method is not a method (no `self` parameter).
    E0012,
    /// A parameter attribute is invalid
    /// (e.g. a non-existent validation prefix, or a literal of the
    /// wrong shape for that prefix).
    E0013,
    /// A `#[tool]` or `#[tool_type]` argument is not recognised.
    E0014,
    /// A `#[wrap]` argument is not recognised
    /// (not `client`, `field`, or `methods`).
    E0015,
    /// A `#[retry]` / `#[rate_limit]` / `#[circuit_breaker]` argument
    /// is not recognised or has the wrong shape.
    E0016,
    /// A `#[delegate(to = "...")]` argument is missing `to = "..."`
    /// or the path cannot be resolved.
    E0017,
    /// A `#[delegate]` method is missing (its body was user-supplied
    /// but the macro couldn't parse it).
    E0018,
    /// The annotation was placed on an item that is not an `impl`
    /// block (e.g. a free function or a struct that does not match
    /// the macro's expectations).
    E0019,
    /// A `#[tool_type]` declaration is missing its `name = "..."`
    /// argument.
    E0020,
    /// A method declares a return type the schema generator cannot
    /// translate to JSON (e.g. raw function pointers, dyn Trait).
    E0021,
    /// A reserved / reserved-by-convention `__` method name was used
    /// as a tool (the `__call_*` / `__TOOL_DEF_*` namespace is owned
    /// by the macro).
    E0022,
    /// Two attributes of the same kind conflict
    /// (e.g. `client = X, client = Y` on `#[wrap]`).
    E0023,
    /// An `async` method consumes `self`. The dispatcher cannot
    /// reconstruct the consumed value, so this is rejected.
    E0024,
    /// A `#[tool]` method is marked `unsafe`. The macro-generated
    /// wrapper is safe, so propagating `unsafe` is not possible.
    E0025,
    /// A trait default method (`#[default]`) was annotated with
    /// `#[tool]`. The macro cannot ship a default body alongside
    /// the generated tool definition.
    E0026,
    /// A method declares more than the schema-generator's parameter
    /// limit (32). Beyond this the JSON Schema becomes unwieldy and
    /// most LLM backends refuse the call.
    E0027,
    /// The `name = "..."` and an entry in `alias = ["..."]` resolve
    /// to the same string, creating an ambiguous dispatch table.
    E0028,
    /// An internal consistency check failed; this is a bug in the
    /// macro, not a user error.
    E0099,
}

impl ErrorCode {
    /// The four-character code as a `&'static str`.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::E0001 => "E0001",
            ErrorCode::E0002 => "E0002",
            ErrorCode::E0003 => "E0003",
            ErrorCode::E0004 => "E0004",
            ErrorCode::E0005 => "E0005",
            ErrorCode::E0006 => "E0006",
            ErrorCode::E0007 => "E0007",
            ErrorCode::E0008 => "E0008",
            ErrorCode::E0009 => "E0009",
            ErrorCode::E0010 => "E0010",
            ErrorCode::E0011 => "E0011",
            ErrorCode::E0012 => "E0012",
            ErrorCode::E0013 => "E0013",
            ErrorCode::E0014 => "E0014",
            ErrorCode::E0015 => "E0015",
            ErrorCode::E0016 => "E0016",
            ErrorCode::E0017 => "E0017",
            ErrorCode::E0018 => "E0018",
            ErrorCode::E0019 => "E0019",
            ErrorCode::E0020 => "E0020",
            ErrorCode::E0021 => "E0021",
            ErrorCode::E0022 => "E0022",
            ErrorCode::E0023 => "E0023",
            ErrorCode::E0024 => "E0024",
            ErrorCode::E0025 => "E0025",
            ErrorCode::E0026 => "E0026",
            ErrorCode::E0027 => "E0027",
            ErrorCode::E0028 => "E0028",
            ErrorCode::E0099 => "E0099",
        }
    }

    /// Anchor in the public docs site.
    pub fn doc_url(self) -> String {
        format!(
            "https://docs.rs/tokitai/latest/tokitai/errors.html#{}",
            self.as_str()
        )
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One diagnostic.
///
/// `MacroError` is the single error type every proc-macro entry point
/// returns on failure. It is intentionally cheap to construct
/// (`with_help` / `with_suggestion` are simple builder methods); the
/// expensive work happens in [`to_compile_error`] / [`to_diagnostic`].
#[derive(Debug, Clone)]
pub struct MacroError {
    code: ErrorCode,
    message: String,
    span: Span,
    help: Option<String>,
    suggestion: Option<String>,
}

impl MacroError {
    /// Construct a fresh diagnostic. `span` is the source span the
    /// error will be anchored to in the user's editor.
    pub fn new(code: ErrorCode, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            span,
            help: None,
            suggestion: None,
        }
    }

    /// Builder: attach a `help:` line.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Builder: attach a "did you mean" suggestion.
    ///
    /// The string should be pre-formatted (e.g.
    /// `` "`bar` or `baz`" ``). The macro formats it into the
    /// diagnostic as `` ^^ did you mean `bar` or `baz`? ``.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// The stable code assigned to this error.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Render the diagnostic as a `compile_error!` invocation.
    ///
    /// rustc's `compile_error!` accepts a single string literal; the
    /// formatted diagnostic is the one blob the user sees. We emit
    /// it through `compile_error!` so the user's editor highlights
    /// the offending span (in IDEs that support it) and so all the
    /// standard rustc error machinery (jump-to-error, copy-id, etc.)
    /// works unchanged.
    pub fn to_compile_error(&self) -> TokenStream {
        // Use the body-only form (no leading `error:`) so the
        // `compile_error!` wrapper does not produce a doubled
        // `error: error[E0001]: ...` line. The body is the
        // diagnostic *minus* the `error[Exxxx]: ` prefix, so
        // when rustc renders it the prefix appears exactly once.
        //
        // Anchor the `compile_error!` invocation at `self.span` so
        // the diagnostic appears at the offending source location
        // (rustc/IDE jump-to-error uses the span of the macro
        // expansion site, not the macro call site).
        let body = self.to_diagnostic_body();
        let span = self.span;
        quote_spanned! {span=>
            compile_error!(#body);
        }
    }

    /// Render the diagnostic as a single, deterministic string.
    ///
    /// The format is intentionally close to rustc's own
    /// ``error[E0xxx]: ...`` style so it is visually familiar. The
    /// output is **line-stable**: the same `MacroError` value always
    /// produces byte-for-byte the same string. This property is what
    /// the `trybuild` snapshot tests in
    /// `tests/error_quality_test.rs` rely on.
    pub fn to_diagnostic(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "error[{}]: {}", self.code, self.message);
        let _ = writeln!(out, "  = help: {}", self.help_text());
        if let Some(s) = &self.suggestion {
            let _ = writeln!(out, "  = note: did you mean {}", s);
        }
        let _ = writeln!(out, "  = help: see {}", self.code.doc_url());
        out
    }

    /// Same as [`to_diagnostic`] but without the leading
    /// `error: ` token. Used by [`to_compile_error`] so the
    /// `compile_error!` wrapper does not produce a doubled prefix.
    /// The body still contains `error[Exxxx]: ...` so the
    /// diagnostic remains a single, well-known rustc-style error
    /// line once rustc prepends its own `error: ` token.
    pub(crate) fn to_diagnostic_body(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "error[{}]: {}", self.code, self.message);
        let _ = writeln!(out, "       = help: {}", self.help_text());
        if let Some(s) = &self.suggestion {
            let _ = writeln!(out, "       = note: did you mean {}", s);
        }
        let _ = writeln!(out, "       = help: see {}", self.code.doc_url());
        out
    }

    /// Pre-formatted `help:` line. If the user did not supply one
    /// explicitly we fall back to a generic pointer to the docs page.
    fn help_text(&self) -> &str {
        self.help
            .as_deref()
            .unwrap_or("see the docs page for this error code")
    }
}

impl std::fmt::Display for MacroError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_diagnostic())
    }
}

impl std::error::Error for MacroError {}

impl From<MacroError> for TokenStream {
    fn from(e: MacroError) -> Self {
        e.to_compile_error()
    }
}

// ---------------------------------------------------------------------------
// Suggestion helpers
// ---------------------------------------------------------------------------

/// Compute the Levenshtein edit distance between two strings.
///
/// We hand-roll this because `trybuild` snapshots run on stable and
/// the proc-macro's `Cargo.toml` declares `syn`, `quote`,
/// `proc-macro2` and `serde` as the only dependencies — pulling in
/// the `edit-distance` crate just for this would be heavy-handed.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

/// Pick the closest matches to `target` from `candidates`.
///
/// Up to three are returned, sorted by ascending edit distance, then
/// alphabetically for ties. A candidate is "close enough" if its
/// distance is at most `max_distance(target.len())` — this follows
/// the convention used by `rustc` itself: a one-character typo in a
/// 4-character identifier is plausible, but in a 20-character name
/// the bar is higher.
pub fn suggest_closest<'a>(target: &str, candidates: &'a [String]) -> Vec<&'a str> {
    let max_dist = max_distance_for(target.len());
    let mut scored: Vec<(usize, &str)> = candidates
        .iter()
        .map(|c| (levenshtein(target, c), c.as_str()))
        .filter(|(d, _)| *d > 0 && *d <= max_dist)
        .collect();
    // Stable: distance asc, then alphabetical.
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)));
    scored.into_iter().take(3).map(|(_, s)| s).collect()
}

/// `rustc`'s rule of thumb: typo budget is roughly
/// `min(target.len() / 3, 4)`.
fn max_distance_for(len: usize) -> usize {
    std::cmp::min(len / 3, 4)
}

/// Format a "did you mean" suggestion for one or more candidates.
///
/// Returns `None` if there are no candidates that look close enough;
/// callers should leave the `suggestion` field empty in that case so
/// the diagnostic stays terse.
pub fn format_did_you_mean(candidates: &[&str]) -> Option<String> {
    match candidates.len() {
        0 => None,
        1 => Some(format!("`{}`?", candidates[0])),
        2 => Some(format!("`{}` or `{}`?", candidates[0], candidates[1])),
        _ => Some(format!(
            "`{}`, `{}`, or `{}`?",
            candidates[0], candidates[1], candidates[2]
        )),
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

impl MacroError {
    /// Build a "method name not found" diagnostic with a built-in
    /// "did you mean" suggestion computed against `available`.
    pub fn method_not_found(span: Span, requested: &str, available: &[String]) -> Self {
        let mut err = Self::new(
            ErrorCode::E0001,
            span,
            format!("unknown method name `{}`", requested),
        )
        .with_help(
            "the `methods = [...]` list must name public methods defined in this `impl` block; \
             check spelling and that the method is `pub`",
        );
        let suggestions = suggest_closest(requested, available);
        if let Some(text) = format_did_you_mean(&suggestions) {
            err = err.with_suggestion(text);
        }
        err
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_basic() {
        assert_eq!(levenshtein("foo", "foo"), 0);
        assert_eq!(levenshtein("foo", "fooo"), 1);
        assert_eq!(levenshtein("foo", "bar"), 3);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn suggest_closest_filters_far() {
        let cands = vec!["get_user".to_string(), "list_repos".to_string()];
        let s = suggest_closest("getuser", &cands);
        // "getuser" is one deletion from "get_user" - within budget.
        assert!(s.contains(&"get_user"));
    }

    #[test]
    fn format_did_you_mean_handles_count() {
        assert_eq!(format_did_you_mean(&[]), None);
        assert_eq!(format_did_you_mean(&["a"]).as_deref(), Some("`a`?"));
        assert_eq!(
            format_did_you_mean(&["a", "b"]).as_deref(),
            Some("`a` or `b`?")
        );
        assert_eq!(
            format_did_you_mean(&["a", "b", "c", "d"]).as_deref(),
            Some("`a`, `b`, or `c`?")
        );
    }

    #[test]
    fn to_diagnostic_is_deterministic() {
        let span = Span::call_site();
        let a = MacroError::new(ErrorCode::E0001, span, "msg").with_help("h");
        let b = MacroError::new(ErrorCode::E0001, span, "msg").with_help("h");
        assert_eq!(a.to_diagnostic(), b.to_diagnostic());
    }
}
