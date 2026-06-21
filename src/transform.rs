//! Row/column transforms: select, drop, head, tail, sort, uniq, clean.
//!
//! Each returns a fresh `Table`; none mutate in place. They're small and
//! composable, which keeps `main.rs` a thin dispatcher.

use crate::table::Table;
use crate::value::{compare_cells, compare_numeric};
use anyhow::Result;
use std::collections::HashSet;

/// Keep only the named columns, in the given order. Reorders and/or narrows.
pub fn select(table: &Table, specs: &str) -> Result<Table> {
    let indices = table.resolve_columns(specs)?;
    project(table, &indices)
}

/// Drop the named columns, preserving the original order of the rest.
pub fn drop(table: &Table, specs: &str) -> Result<Table> {
    let to_drop: HashSet<usize> = table.resolve_columns(specs)?.into_iter().collect();
    let keep: Vec<usize> = (0..table.width())
        .filter(|i| !to_drop.contains(i))
        .collect();
    project(table, &keep)
}

/// Build a new table consisting of the given column indices.
fn project(table: &Table, indices: &[usize]) -> Result<Table> {
    let headers = indices.iter().map(|&i| table.headers[i].clone()).collect();
    let rows = table
        .rows
        .iter()
        .map(|row| {
            indices
                .iter()
                .map(|&i| table.cell(row, i).to_string())
                .collect()
        })
        .collect();
    Ok(Table::new(headers, rows))
}

/// First `n` rows. `n >= len` returns all rows.
pub fn head(table: &Table, n: usize) -> Table {
    let rows = table.rows.iter().take(n).cloned().collect();
    Table::new(table.headers.clone(), rows)
}

/// Last `n` rows. `n >= len` returns all rows.
pub fn tail(table: &Table, n: usize) -> Table {
    let start = table.rows.len().saturating_sub(n);
    let rows = table.rows[start..].to_vec();
    Table::new(table.headers.clone(), rows)
}

/// Sort rows by a single column.
///
/// `numeric` forces numeric ordering (non-numbers sort first); otherwise we use
/// the auto rule (numeric when both cells parse, else lexical) per pair. `desc`
/// reverses. The sort is stable so equal keys keep input order.
pub fn sort(table: &Table, by: &str, desc: bool, numeric: bool) -> Result<Table> {
    let idx = table.resolve_column(by)?;
    let mut rows = table.rows.clone();
    rows.sort_by(|a, b| {
        let av = a.get(idx).map(|s| s.as_str()).unwrap_or("");
        let bv = b.get(idx).map(|s| s.as_str()).unwrap_or("");
        let ord = if numeric {
            compare_numeric(av, bv)
        } else {
            compare_cells(av, bv)
        };
        if desc {
            ord.reverse()
        } else {
            ord
        }
    });
    Ok(Table::new(table.headers.clone(), rows))
}

/// Deduplicate rows. With no `by`, dedup on the entire row; with `by`, dedup on
/// the tuple of those columns' values. First occurrence is kept; input order is
/// otherwise preserved.
pub fn uniq(table: &Table, by: Option<&str>) -> Result<Table> {
    let indices: Option<Vec<usize>> = match by {
        Some(specs) => Some(table.resolve_columns(specs)?),
        None => None,
    };

    let mut seen: HashSet<Vec<String>> = HashSet::new();
    let mut out: Vec<Vec<String>> = Vec::new();

    for row in &table.rows {
        let key: Vec<String> = match &indices {
            Some(idxs) => idxs
                .iter()
                .map(|&i| table.cell(row, i).to_string())
                .collect(),
            None => row.clone(),
        };
        if seen.insert(key) {
            out.push(row.clone());
        }
    }
    Ok(Table::new(table.headers.clone(), out))
}

/// Tidy a table: trim leading/trailing whitespace from every cell and drop rows
/// that are entirely empty after trimming. Line-ending normalization is handled
/// for free by the CSV writer (it emits `\r\n`-free records via our settings).
pub fn clean(table: &Table) -> Table {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in &table.rows {
        let trimmed: Vec<String> = row.iter().map(|c| c.trim().to_string()).collect();
        if trimmed.iter().all(|c| c.is_empty()) {
            continue;
        }
        rows.push(trimmed);
    }
    // Trim headers too, so a header like " name " becomes "name".
    let headers = table.headers.iter().map(|h| h.trim().to_string()).collect();
    Table::new(headers, rows)
}
