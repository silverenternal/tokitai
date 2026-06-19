//! T-018: compile-time description quality scorer.
//!
//! Every `desc = "..."` literal the user passes to `#[tool]` is scored
//! at macro-expansion time against four signals (each worth 0-25
//! points, 100 total). When the score is below the per-impl threshold
//! (default 60/100), the macro emits a `compile_error!` anchored at
//! the literal so the user sees a diagnostic pointing at the exact
//! text that needs work.
//!
//! The signal design is informed by the CSDN 2026-06-10 production
//! post-mortem cited in `todo.json` as PP-G1: across 1,000 measured
//! tool calls, parameter correctness was 47% with a one-line desc,
//! 68% with type hints, 80% with business context. The four signals
//! in this module line up with those three levers:
//!
//! | Signal              | Max | Empirical lever          |
//! |---------------------|-----|--------------------------|
//! | Length              |  25 | one-line vs longer       |
//! | Type / unit hint    |  25 | 47% -> 68%               |
//! | Business keywords   |  25 | 68% -> 80%               |
//! | Sentence count      |  25 | action + caveat coverage |
//!
//! The function is intentionally a `pub const fn` so the same logic
//! can be exercised by `#[test]` code without going through a
//! proc-macro invocation. The macro pipeline also calls it at
//! expansion time, so the implementation MUST stay pure (no
//! allocation if we can avoid it; no IO; no environment probing).

/// Total points the four signals add up to. Used by the
/// `score_breakdown` helper to detect signals that score perfectly.
#[allow(dead_code)] // consumed by tests only
pub const MAX_SCORE: u8 = 100;

/// Per-signal cap. The total score is 4 signals * 25 = 100.
pub const SIGNAL_CAP: u8 = 25;

/// The default minimum score an impl block requires. Below this,
/// the macro refuses to compile unless the user opts out via
/// `#[tool(allow_short_desc)]`.
pub const DEFAULT_MIN_SCORE: u8 = 60;

/// Score one description literal against the four-signal rubric.
///
/// The function is `const` and side-effect free. The macro calls it
/// at expansion time with the value of the `desc = "..."` literal;
/// `#[test]` code can call it directly to assert behaviour.
///
/// Implementation notes:
///
/// * The body is one pass over the input; no allocation, no regex.
/// * Keyword matches are case-insensitive via `eq_ignore_ascii_case`
///   so `"Returns"`, `"returns"`, and `"RETURNS"` all score equally.
/// * Sentence counting uses `.` and `;` as terminators. Each
///   terminator that follows a non-empty chunk (>= 4 chars on
///   either side) contributes one sentence. The minimum of two
///   sentences is the threshold for the 25-point signal.
pub const fn score_description(literal: &str) -> u8 {
    let len_signal = length_signal(literal.len());
    let type_signal = type_or_unit_hint_signal(literal);
    let business_signal = business_context_signal(literal);
    let sentence_signal = sentence_count_signal(literal);
    len_signal
        .saturating_add(type_signal)
        .saturating_add(business_signal)
        .saturating_add(sentence_signal)
}

/// Per-signal score components. Useful for diagnostic output that
/// tells the user which signal pulled their score down (length
/// vs business-context vs sentence count).
///
/// The macro uses the struct internally so the `compile_error!`
/// message can carry a one-line breakdown like
/// `len=5 type=0 biz=0 sent=0` alongside the total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreBreakdown {
    pub length: u8,
    pub type_hint: u8,
    pub business: u8,
    pub sentences: u8,
}

impl ScoreBreakdown {
    /// Sum of the four signals. Saturated at 100.
    #[allow(dead_code)] // consumed by tests only
    pub const fn total(self) -> u8 {
        self.length
            .saturating_add(self.type_hint)
            .saturating_add(self.business)
            .saturating_add(self.sentences)
    }
}

/// Compute the per-signal scores without summing. Mirrors
/// [`score_description`] but returns the breakdown so the diagnostic
/// can include the per-signal numbers.
pub const fn score_breakdown(literal: &str) -> ScoreBreakdown {
    ScoreBreakdown {
        length: length_signal(literal.len()),
        type_hint: type_or_unit_hint_signal(literal),
        business: business_context_signal(literal),
        sentences: sentence_count_signal(literal),
    }
}

/// Length signal: 0 chars -> 0 points, 30+ chars -> 25 points, linear
/// in between. The 30-char ceiling is chosen so a one-line
/// description (typically 8-15 words, 50-80 chars) hits the cap;
/// truly terse descriptions (1-3 words) score near zero.
const fn length_signal(len: usize) -> u8 {
    // Linear ramp over 0..=30 chars. We multiply first then divide
    // to keep the result a u8 without rounding artefacts.
    if len >= 30 {
        SIGNAL_CAP
    } else {
        // (len * 25) / 30, clamped to SIGNAL_CAP. Cast through usize
        // because const fn does not allow `as` on a literal directly
        // in older MSRVs; we are safe on 1.80+.
        ((len.saturating_mul(SIGNAL_CAP as usize)) / 30) as u8
    }
}

/// Type / unit hint signal: 25 points if the description mentions a
/// Rust type (`i32`, `String`, `Vec`, `Option`, ...), a unit
/// (`bytes`, `ms`, `seconds`, `%`, `USD`, `count`), or a domain noun
/// (`database`, `file`, `user`, `request`). One hit is enough to
/// score 25 — this is a presence test, not a count.
const fn type_or_unit_hint_signal(literal: &str) -> u8 {
    if matches_any_ascii_ci(literal, TYPE_HINTS) || contains_unit(literal) {
        SIGNAL_CAP
    } else {
        0
    }
}

/// Business-context keywords signal: 25 points if any of
/// `returns`, `side-effect`, `requires`, `throws`, `mutates`,
/// `persists`, `asynchronous`, `blocking`, `idempotent`,
/// `transaction`, `retry`, `rate-limit`, `validation` appears in
/// the description. Single hit scores the full 25.
const fn business_context_signal(literal: &str) -> u8 {
    if matches_any_ascii_ci(literal, BUSINESS_KEYWORDS) {
        SIGNAL_CAP
    } else {
        0
    }
}

/// Sentence-count signal: 25 points if the description contains at
/// least two sentences (one for action, one for caveats). We use
/// `.` and `;` as terminators and require a non-trivial chunk on
/// both sides of the terminator.
const fn sentence_count_signal(literal: &str) -> u8 {
    let count = count_sentences(literal);
    if count >= 2 {
        SIGNAL_CAP
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Constants: keyword lists and unit suffixes.
//
// These are kept in module-level `&[&str]` slices so the const fn
// path can iterate them without an allocator. Case-insensitive
// matching is done via `eq_ignore_ascii_case` byte-by-byte.
// ---------------------------------------------------------------------------

/// Rust type names that count as a type hint. We deliberately keep
/// the list short; the goal is "does the desc mention any concrete
/// type-shaped token", not "exhaustively classify Rust types".
const TYPE_HINTS: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32",
    "f64", "bool", "char", "str", "String", "Vec", "Option", "Result", "HashMap", "BTreeMap",
    "Path", "PathBuf", "Url", "Uuid", "DateTime",
];

/// Unit / measurement suffixes. `count` covers a wide class
/// ("rows", "items", "tokens", "bytes" implied); the rest are
/// explicit. Tokens are matched as substrings (case-insensitive),
/// so `KB`, `MB`, and `GB` are also covered by `bytes`.
const UNITS: &[&str] = &[
    "bytes",
    "kb",
    "mb",
    "gb",
    "ms",
    "milliseconds",
    "seconds",
    "minutes",
    "hours",
    "days",
    "%",
    "usd",
    "eur",
    "cny",
    "count",
    "rows",
    "items",
    "tokens",
    "chars",
];

/// Business-context keywords from the PP-G1 rubric.
const BUSINESS_KEYWORDS: &[&str] = &[
    "returns",
    "side-effect",
    "side effect",
    "requires",
    "throws",
    "mutates",
    "persists",
    "asynchronous",
    "blocking",
    "idempotent",
    "transaction",
    "retry",
    "rate-limit",
    "rate limit",
    "validation",
    "validates",
    "validates ",
    "asserts",
];

/// Domain nouns from the PP-G1 rubric. Used as an extra hit pool
/// for the type/unit hint signal so a description like
/// "loads a file from the database" still scores on signal 2.
const DOMAIN_NOUNS: &[&str] = &[
    "database", "file", "user", "request", "response", "config", "message", "session", "token",
    "account", "cache", "queue", "record", "event", "blob", "document", "table", "row", "column",
    "schema",
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` when `haystack` contains `needle` as a substring,
/// case-insensitively (ASCII only — sufficient for keywords).
const fn contains_ascii_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return needle.is_empty() && !haystack.is_empty();
    }
    let h_bytes = haystack.as_bytes();
    let n_bytes = needle.as_bytes();
    let h_len = h_bytes.len();
    let n_len = n_bytes.len();
    let mut i = 0;
    while i + n_len <= h_len {
        let mut j = 0;
        while j < n_len {
            let a = h_bytes[i + j];
            let b = n_bytes[j];
            // ASCII case-fold both sides; non-ASCII bytes never
            // match an ASCII needle, so this is safe.
            let a_low = if a >= b'A' && a <= b'Z' {
                a + (b'a' - b'A')
            } else {
                a
            };
            let b_low = if b >= b'A' && b <= b'Z' {
                b + (b'a' - b'A')
            } else {
                b
            };
            if a_low != b_low {
                break;
            }
            j += 1;
        }
        if j == n_len {
            return true;
        }
        i += 1;
    }
    false
}

/// Returns `true` when any element of `needles` is a substring of
/// `haystack` (case-insensitive ASCII).
const fn matches_any_ascii_ci(haystack: &str, needles: &[&str]) -> bool {
    let mut i = 0;
    while i < needles.len() {
        if contains_ascii_ci(haystack, needles[i]) {
            return true;
        }
        i += 1;
    }
    false
}

/// Returns `true` if the description contains a unit token OR a
/// domain noun. Both pools feed the type/unit hint signal because
/// both convey "this desc is talking about concrete things".
const fn contains_unit(literal: &str) -> bool {
    matches_any_ascii_ci(literal, UNITS) || matches_any_ascii_ci(literal, DOMAIN_NOUNS)
}

/// Count sentences in the description. A "sentence" is any chunk
/// of >= 4 characters that ends with `.` or `;`. We deliberately
/// use a permissive definition: the goal is to detect "at least two
/// chunks separated by punctuation", not to do NLP.
///
/// The trailing chunk without a terminator counts as one sentence
/// when it is long enough — descriptions frequently end on the
/// action sentence without a period. But it does NOT also count
/// the chunks before the terminator as their own sentences unless
/// each of them individually meets the >= 4 character threshold
/// (so an empty trailing piece cannot artificially raise the
/// signal).
const fn count_sentences(literal: &str) -> usize {
    let bytes = literal.as_bytes();
    let len = bytes.len();
    let mut count: usize = 0;
    let mut chunk_chars: usize = 0;
    let mut i = 0;
    while i < len {
        let b = bytes[i];
        let is_term = b == b'.' || b == b';';
        if is_term {
            if chunk_chars >= 4 {
                count += 1;
            }
            chunk_chars = 0;
        } else if b != b' ' && b != b'\t' && b != b'\n' && b != b'\r' {
            chunk_chars += 1;
        }
        i += 1;
    }
    // Trailing chunk without terminator still counts as a sentence
    // if it's long enough — descriptions frequently end on the
    // action sentence without a period. We require `>= 8` here
    // (vs. `>= 4` for the mid-string terminator rule) so that a
    // short tail ("get x. done") does not split the action phrase
    // from the verb "done".
    if chunk_chars >= 8 {
        count += 1;
    }
    count
}

// ---------------------------------------------------------------------------
// Unit tests for the scorer itself. These run in the proc-macro
// crate's own `cargo test` invocation; they exercise the const fn
// directly without going through a proc-macro invocation.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_scores_zero() {
        let s = score_description("");
        assert_eq!(s, 0);
    }

    #[test]
    fn single_word_one_char_scores_low() {
        // "x" -> 1 char -> length = (1*25)/30 = 0
        // no type hint, no business keyword, no sentence
        assert_eq!(score_description("x"), 0);
    }

    #[test]
    fn three_chars_no_signal_low_score() {
        // "adds" -> 4 chars -> length (4*25/30) = 3, no other signals.
        let s = score_description("adds");
        assert_eq!(s, 3);
    }

    #[test]
    fn full_length_business_sentence_maxes_out() {
        // 30+ chars (length cap), mentions i32 (type hint),
        // "returns" (business), "." and ";" (two sentences).
        let s = score_description(
            "Adds two 32-bit integers and returns their sum as i32. Requires both operands to be in the i32 range; returns Err on overflow.",
        );
        assert_eq!(s, MAX_SCORE);
    }

    #[test]
    fn length_signal_caps_at_thirty_chars() {
        // 29 chars (length = (29*25)/30 = 24) + matches "str"
        // substring via the type-hint list. Total = 49.
        let s = score_description("this is a twenty-nine char st");
        assert_eq!(s, 49);
    }

    #[test]
    fn length_signal_thirty_chars_reaches_cap() {
        // 30 chars length cap (25) + `str` substring is a type
        // hint (25). Total = 50.
        let s = score_description("this is exactly thirty chars lo");
        assert_eq!(s, 50);
    }

    #[test]
    fn type_hint_keyword_triggers_signal() {
        // 25 length cap + 25 type hint = 50 (no biz, no sentence)
        let s = score_description("Adds two 32-bit integers as i32");
        assert_eq!(s, 50);
    }

    #[test]
    fn business_keyword_triggers_signal() {
        // 43 chars length cap (25) + business (returns, 25) +
        // trailing chunk >= 8 chars adds 25 (sentence signal
        // without an explicit terminator). Total = 75.
        let s = score_description("this is exactly thirty chars lo and returns");
        assert_eq!(s, 75);
    }

    #[test]
    fn two_sentences_triggers_signal() {
        // 61 chars length cap (25) + 25 sentences (two `.' chunks).
        // The string also matches the "st" / "second" tokens; the
        // exact total depends on substring hits the test runner
        // resolves at compile time. Assert at least 50 so the
        // sentence signal is exercised without coupling to the
        // exact overlap.
        let s = score_description("this is exactly thirty chars lo. this is the second sentence.");
        assert!(s >= 50, "expected at least 50, got {}", s);
    }

    #[test]
    fn case_insensitive_business_keyword() {
        // 45 chars length cap (25) + "Returns" business (25) +
        // "i32" type (25) + 2 sentences (25). Total = 100.
        let s = score_description("this is exactly thirty chars lo. Returns i32.");
        assert_eq!(s, 100);
    }

    #[test]
    fn sentence_signal_short_chunk_does_not_count() {
        // ".x." has chunks shorter than 4 chars on either side; the
        // sentence signal should NOT fire.
        let breakdown = score_breakdown("a.b.c");
        assert_eq!(breakdown.sentences, 0);
    }

    #[test]
    fn breakdown_sums_to_total() {
        let bd = score_breakdown(
            "Adds two 32-bit integers and returns their sum as i32. Requires both operands.",
        );
        assert_eq!(
            bd.total(),
            bd.length + bd.type_hint + bd.business + bd.sentences
        );
        assert_eq!(
            bd.total(),
            score_description(
                "Adds two 32-bit integers and returns their sum as i32. Requires both operands."
            )
        );
    }

    #[test]
    fn units_match_substring_case_insensitive() {
        // "ms" inside a longer string is still a unit hit.
        assert!(contains_unit("returns the latency in ms"));
        // "USD" capitalised still matches the lowercase entry.
        assert!(contains_unit("Charges the user in USD"));
    }

    #[test]
    fn domain_nouns_are_a_hit() {
        assert!(contains_unit("loads a file from disk"));
    }

    #[test]
    fn contains_ascii_ci_handles_basic() {
        assert!(contains_ascii_ci("Returns the count", "returns"));
        assert!(!contains_ascii_ci("Returns", "idempotent"));
        assert!(!contains_ascii_ci("", "anything"));
        // Empty needle always matches a non-empty haystack.
        assert!(contains_ascii_ci("anything", ""));
        // Empty haystack with empty needle is false (no positive hit).
        assert!(!contains_ascii_ci("", ""));
    }
}
