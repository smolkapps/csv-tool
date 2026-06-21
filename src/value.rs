//! Value comparison helpers shared by `filter` and `sort`.
//!
//! The rule from the spec: compare numerically when *both* operands parse as
//! numbers, otherwise compare as strings. We centralize that decision here so
//! filter and sort behave identically.

use std::cmp::Ordering;

/// Try to parse a cell as an f64, trimming surrounding whitespace. Empty cells
/// are treated as non-numeric (so they fall back to string compare and don't
/// silently become 0).
pub fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok()
}

/// Compare two cells. If both parse as numbers, compare numerically; otherwise
/// compare lexicographically. NaN never appears because `parse_num` rejects
/// anything `f64::from_str` wouldn't accept, but we still guard the partial_cmp.
pub fn compare_cells(a: &str, b: &str) -> Ordering {
    match (parse_num(a), parse_num(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => a.cmp(b),
    }
}

/// Force a numeric comparison (used by `sort --numeric`). Non-numeric cells sort
/// before numeric ones and among themselves lexically, which keeps the sort
/// total and deterministic.
pub fn compare_numeric(a: &str, b: &str) -> Ordering {
    match (parse_num(a), parse_num(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => a.cmp(b),
    }
}
