//! Value counts for a single column — the `frequency` subcommand.
//!
//! Tallies how often each distinct value appears in one column and returns a
//! two-column `value,count` table. Rows are ordered by descending count, then by
//! value ascending as a tie-breaker, so the output is deterministic regardless
//! of input order. This complements `stats` (which summarizes a whole column)
//! by answering "which values are most common?".

use crate::table::Table;
use anyhow::Result;
use std::collections::HashMap;

/// Count occurrences of each distinct value in the column named by `spec`.
///
/// The value is taken verbatim (ragged rows yield an empty-string value, matching
/// the rest of the tool). Output columns are always `value,count`.
pub fn frequency(table: &Table, spec: &str) -> Result<Table> {
    let idx = table.resolve_column(spec)?;

    // Tally counts. A HashMap has no iteration order, so the sort below — which
    // breaks count ties by value ascending (a total order over distinct values) —
    // is what makes the output fully deterministic regardless of input order.
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for row in &table.rows {
        *counts.entry(table.cell(row, idx)).or_insert(0) += 1;
    }

    let mut pairs: Vec<(&str, usize)> = counts.into_iter().collect();
    // Most frequent first; ties broken by value ascending for stable output.
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let headers = vec!["value".to_string(), "count".to_string()];
    let rows = pairs
        .into_iter()
        .map(|(value, n)| vec![value.to_string(), n.to_string()])
        .collect();
    Ok(Table::new(headers, rows))
}
