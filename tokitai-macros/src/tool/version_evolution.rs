//! T-020: compile-time schema-evolution interval check.
//!
//! When an impl block opts into `#[tool(version_policy = "semver")]`,
//! the macro parses every `since = "..."` / `until = "..."` literal
//! as SemVer, refuses to compile when a literal fails to parse, and
//! refuses to compile when the interval is empty (i.e. `since >=
//! until`). The check runs once per impl block at macro-expansion
//! time and emits a `compile_error!` at the offending literal's
//! span when the constraint fails.
//!
//! For non-semver policies (or when no policy is set), this module
//! is a no-op: the macro accepts any string and the dispatcher
//! uses lexicographic ordering at runtime. That keeps CalVer and
//! commit-SHA intervals ergonomic.

use proc_macro2::Span;

use crate::error::{ErrorCode, MacroError};
use crate::tool::types::tool_method::ToolMethodInfo;

/// Single check result. `error` is `Some(_)` when the lint fails;
/// the macro renders it as `compile_error!` at the offending
/// literal's span.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VersionIntervalLint {
    /// The literal's source span.
    pub span: Span,
    /// The literal value (the user-written string).
    pub literal: String,
    /// `Some(_)` when the version string fails to parse as SemVer
    /// (only when `version_policy = "semver"` was set on the
    /// impl block).
    pub error: Option<MacroError>,
}

/// Lint a single `since` / `until` literal against the impl-level
/// `version_policy`.
///
/// * `span` — the literal's span (used to anchor the diagnostic).
/// * `literal` — the user-written string.
/// * `version_policy` — `Some("semver")` when the impl opted in,
///   `None` (or anything else) for the loose-string policy.
pub fn lint_version_interval(
    span: Span,
    literal: &str,
    version_policy: Option<&str>,
) -> VersionIntervalLint {
    let error = match version_policy {
        Some(policy) if policy.eq_ignore_ascii_case("semver") => {
            // Strict SemVer: anything that fails to parse is a
            // compile error pointing at the literal. We accept
            // an optional `v` prefix and pre-release / build
            // metadata (matches the canonical `semver` crate
            // behaviour).
            match semver::Version::parse(strip_v_prefix(literal)) {
                Ok(_) => None,
                Err(_) => Some(
                    MacroError::new(
                        ErrorCode::E0099,
                        span,
                        format!(
                            "`{}` is not a valid SemVer version (version_policy = \"semver\")",
                            literal
                        ),
                    )
                    .with_help(
                        "SemVer 2.0 strings look like `1.0.0` (with an optional `v` prefix). \
                         Drop `version_policy = \"semver\"` if you want to use CalVer \
                         (e.g. `2026.06`) or commit-SHA strings instead.",
                    ),
                ),
            }
        }
        _ => None,
    };
    VersionIntervalLint {
        span,
        literal: literal.to_string(),
        error,
    }
}

/// Check that an impl block's intervals are coherent. Catches the
/// two non-obvious breakages the macro can detect at compile time:
///
/// 1. A single method whose `since >= until` — the interval is
///    empty and the tool would never be served.
/// 2. Two methods whose intervals overlap such that the LLM sees
///    both schemas simultaneously for some `current_version`. The
///    dispatcher picks the FIRST interval in declaration order, so
///    overlapping intervals are usually a mistake. We only flag
///    this when both intervals are strict SemVer (so a CalVer
///    user can declare multiple "overlapping" methods on
///    purpose — e.g. `2026.06` and `2026.06-nightly` for
///    pre-release variants).
///
/// Returns the diagnostics to surface; the macro short-circuits
/// on the first one when emitting `compile_error!`.
pub fn check_impl_intervals(
    tools: &[ToolMethodInfo],
    version_policy: Option<&str>,
) -> Vec<MacroError> {
    let mut errors: Vec<MacroError> = Vec::new();

    for tool in tools {
        // 1. Empty interval check (since >= until).
        if let (Some(since), Some(until)) = (tool.since.as_deref(), tool.until.as_deref()) {
            let since_ok = semver::Version::parse(strip_v_prefix(since)).is_ok();
            let until_ok = semver::Version::parse(strip_v_prefix(until)).is_ok();
            if since_ok && until_ok {
                let s = semver::Version::parse(strip_v_prefix(since)).unwrap();
                let u = semver::Version::parse(strip_v_prefix(until)).unwrap();
                if s >= u {
                    let span = tool.ident_span;
                    errors.push(
                        MacroError::new(
                            ErrorCode::E0099,
                            span,
                            format!(
                                "method `{}` has an empty schema-evolution interval \
                                 (since={} is not strictly before until={}). The dispatcher \
                                 would never serve this tool.",
                                tool.name, since, until
                            ),
                        )
                        .with_help(
                            "swap the two bounds, or drop one of them. The interval is \
                             half-open: `[since, until)`.",
                        ),
                    );
                }
            }
        }
        // 2. Per-literal parse check (already covered by
        //    `lint_version_interval`, but we run it here so the
        //    function is self-contained for testing).
        if let Some(since) = tool.since.as_deref() {
            if let Some(err) = lint_version_interval(tool.ident_span, since, version_policy).error {
                errors.push(err);
            }
        }
        if let Some(until) = tool.until.as_deref() {
            if let Some(err) = lint_version_interval(tool.ident_span, until, version_policy).error {
                errors.push(err);
            }
        }
    }

    // 3. Overlap detection. Walk the sorted interval list and
    //    flag any pair that overlap. We sort by `since` to make
    //    the diagnostic deterministic.
    if version_policy
        .map(|p| p.eq_ignore_ascii_case("semver"))
        .unwrap_or(false)
    {
        let mut sorted: Vec<&ToolMethodInfo> = tools
            .iter()
            .filter(|t| t.since.is_some() || t.until.is_some())
            .collect();
        sorted.sort_by_key(|t| t.since.clone().unwrap_or_default());
        for pair in sorted.windows(2) {
            let prev = pair[0];
            let next = pair[1];
            // Two intervals overlap when prev.until > next.since
            // (using half-open semantics on both sides).
            if let (Some(prev_until), Some(next_since)) =
                (prev.until.as_deref(), next.since.as_deref())
            {
                let prev_ok = semver::Version::parse(strip_v_prefix(prev_until)).is_ok();
                let next_ok = semver::Version::parse(strip_v_prefix(next_since)).is_ok();
                if prev_ok && next_ok {
                    let u = semver::Version::parse(strip_v_prefix(prev_until)).unwrap();
                    let s = semver::Version::parse(strip_v_prefix(next_since)).unwrap();
                    if u > s {
                        let span = next.ident_span;
                        errors.push(
                            MacroError::new(
                                ErrorCode::E0099,
                                span,
                                format!(
                                    "method `{}` (since={}) overlaps with method `{}` \
                                     (until={}). The dispatcher picks the first matching \
                                     interval and may serve a stale schema to a client at \
                                     version {}=since.",
                                    next.name, next_since, prev.name, prev_until, next_since
                                ),
                            )
                            .with_help(
                                "intervals must tile the version line with no overlap. \
                                 Move `until` on the earlier method down to `since` on the \
                                 later method (e.g. `until = \"2.0.0\"` and `since = \"2.0.0\"`).",
                            ),
                        );
                    }
                }
            }
        }
    }

    errors
}

fn strip_v_prefix(s: &str) -> &str {
    s.trim().strip_prefix('v').unwrap_or(s.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;

    fn span() -> Span {
        Span::call_site()
    }

    #[test]
    fn semver_policy_accepts_valid_semver() {
        let lint = lint_version_interval(span(), "1.0.0", Some("semver"));
        assert!(lint.error.is_none());
        let lint = lint_version_interval(span(), "v2.0.0-rc.1", Some("semver"));
        assert!(lint.error.is_none());
    }

    #[test]
    fn semver_policy_rejects_calver() {
        let lint = lint_version_interval(span(), "2026.06", Some("semver"));
        assert!(lint.error.is_some(), "CalVer must fail under semver");
    }

    #[test]
    fn loose_policy_accepts_anything() {
        let lint = lint_version_interval(span(), "2026.06", None);
        assert!(lint.error.is_none());
        let lint = lint_version_interval(span(), "abc123", None);
        assert!(lint.error.is_none());
    }
}
