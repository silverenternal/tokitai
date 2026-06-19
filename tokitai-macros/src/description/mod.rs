//! T-018: compile-time description quality scorer (entry point).
//!
//! The macro pipeline reaches into this module when a `desc = "..."`
//! literal is found on a `#[tool(...)]` attribute. The single public
//! entry point is [`lint_description`], which:
//!
//! 1. Computes the score via the [`score`] module's `const fn`.
//! 2. Compares the score against the per-impl threshold (default
//!    60/100, set via `#[tool(min_desc_score = N)]`).
//! 3. Returns a [`DescriptionLint`] the caller can inspect to
//!    either emit a `compile_error!` or pass the literal through
//!    unchanged.
//!
//! The `#[tool(allow_short_desc)]` flag short-circuits the lint so
//! users can opt out for the rare case where brevity is the point.

use proc_macro2::Span;

pub mod score;

use crate::error::{ErrorCode, MacroError};

/// A single description lint result. The macro uses the `error`
/// field to decide whether to emit a `compile_error!` at the
/// `desc =` literal's span.
#[derive(Debug, Clone)]
#[allow(dead_code)] // some fields are consumed only by tests
pub struct DescriptionLint {
    /// The literal's source span (used to anchor the diagnostic).
    pub span: Span,
    /// The literal value (after unescape). Stored so the macro can
    /// pass it through unchanged when the score passes the bar.
    pub literal: String,
    /// The computed score (0-100).
    pub score: u8,
    /// The threshold in effect at this site.
    pub threshold: u8,
    /// `Some(_)` when the score is below the threshold and the
    /// user did NOT opt out via `allow_short_desc`. The macro
    /// renders the inner `MacroError` via `to_compile_error()`.
    pub error: Option<MacroError>,
}

/// Lint a `desc = "..."` literal.
///
/// * `span` — the `LitStr`'s span so the diagnostic anchors at
///   the literal in the user's source.
/// * `literal` — the unescaped string value.
/// * `threshold` — the per-impl minimum score. Pass
///   [`score::DEFAULT_MIN_SCORE`] for the standard 60/100 bar.
/// * `allow_opt_out` — when `true`, the lint short-circuits and
///   the literal is always accepted (with `error = None`). Used
///   to honour `#[tool(allow_short_desc)]`.
pub fn lint_description(
    span: Span,
    literal: &str,
    threshold: u8,
    allow_opt_out: bool,
) -> DescriptionLint {
    let score = score::score_description(literal);
    let breakdown = score::score_breakdown(literal);

    let error = if allow_opt_out || score >= threshold {
        None
    } else {
        // Build a diagnostic anchored at the literal's span. The
        // body carries the per-signal breakdown so the user can see
        // which signal pulled their score below the bar.
        let body = format!(
            "tool description scores {}/100; minimum is {}/100 (length={}/25, \
             type/unit={}/25, business-context={}/25, sentences={}/25)",
            score,
            threshold,
            breakdown.length,
            breakdown.type_hint,
            breakdown.business,
            breakdown.sentences,
        );
        Some(
            MacroError::new(ErrorCode::E0031, span, body).with_help(format!(
                "expand the description: add a concrete type or unit hint, mention what the \
                 tool returns / requires / mutates / persists, and split the action from its \
                 caveats into two sentences. Pass `#[tool(allow_short_desc)]` to opt out for \
                 one-word verbs, or `#[tool(min_desc_score = {})]` to lower the threshold for \
                 this impl.",
                threshold,
            )),
        )
    };

    DescriptionLint {
        span,
        literal: literal.to_string(),
        score,
        threshold,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_long_description() {
        let span = Span::call_site();
        let lint = lint_description(
            span,
            "Adds two 32-bit integers and returns their sum as i32. Requires both operands.",
            score::DEFAULT_MIN_SCORE,
            false,
        );
        assert!(lint.error.is_none(), "long desc should pass: {:?}", lint);
        assert!(lint.score >= 60);
    }

    #[test]
    fn fails_three_char_desc() {
        let span = Span::call_site();
        let lint = lint_description(span, "adds", score::DEFAULT_MIN_SCORE, false);
        let err = lint.error.expect("short desc should fail");
        let body = err.to_diagnostic_body();
        // "adds" is 4 chars; length signal = (4*25)/30 = 3. Score
        // is therefore 3/100; the threshold is 60/100.
        assert!(body.contains("scores 3/100"), "got: {}", body);
        assert!(body.contains("minimum is 60/100"), "got: {}", body);
    }

    #[test]
    fn allow_short_desc_opt_out_works() {
        let span = Span::call_site();
        let lint = lint_description(span, "x", score::DEFAULT_MIN_SCORE, true);
        assert!(lint.error.is_none(), "opt-out should bypass the lint");
    }

    #[test]
    fn lowered_threshold_accepts_short_desc() {
        let span = Span::call_site();
        // 3 chars scores 2; threshold 2 should pass.
        let lint = lint_description(span, "adds", 2, false);
        assert!(lint.error.is_none(), "lowered threshold should pass");
    }
}
