//! Blank-slate consumer crate used as the canonical fixture for
//! `scripts/measure-consumer-impact.sh`.
//!
//! It contains a small handful of plain functions and a single
//! dependency on `tokitai`, but **no `#[tool]` impl blocks**. The
//! measurement script synthesises its own `#[tool]` impl blocks
//! and injects them on top of this baseline so the per-impl-block
//! compile-time overhead can be attributed to the macro itself,
//! not to the user's existing code.
//!
//! See `scripts/README.md` and
//! `docs/internal/consumer-compile-time-impact.md` for details.

use serde_json::{json, Value};

/// Returns the string `"hello, world"` as a `serde_json::Value`.
///
/// Lives outside any `#[tool]` block so it does not bias the
/// baseline measurement.
pub fn greet() -> Value {
    json!("hello, world")
}

/// Doubles an integer. Plain function — no macro involvement.
pub fn double(n: i64) -> i64 {
    n.saturating_mul(2)
}

/// Trivial `add` operation. Plain function — no macro involvement.
pub fn add(a: i64, b: i64) -> i64 {
    a.saturating_add(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet(), json!("hello, world"));
    }

    #[test]
    fn test_double() {
        assert_eq!(double(21), 42);
    }

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
