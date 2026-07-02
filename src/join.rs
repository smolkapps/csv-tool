//! Join two CSV tables on a key column for the `join` subcommand.
//!
//! The left table is the main input (stdin or `-i`); the right table is a file
//! given on the command line. Output columns are all of the left table's columns
//! followed by the right table's columns *except* its key column — so the key
//! appears once and both tables' original column order is preserved. Rows that
//! share a key form the cartesian product per key (duplicate keys multiply),
//! mirroring a SQL join.

use crate::table::Table;
use anyhow::Result;
use std::collections::HashMap;

/// What to do with left rows whose key has no match on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// Keep only rows whose key matches on both sides (SQL `INNER JOIN`).
    Inner,
    /// Keep every left row; unmatched ones get empty cells for the right
    /// columns (SQL `LEFT OUTER JOIN`).
    Left,
}

/// Join `left` and `right` on a key column.
///
/// `left_on`/`right_on` name the key column in each table (by header name or
/// 0-based index, resolved independently) so the two files may use different
/// column names for the same key. Right rows are indexed by key value for a
/// single linear pass over the left table.
pub fn join(left: &Table, right: &Table, left_on: &str, right_on: &str, how: How) -> Result<Table> {
    let lkey = left.resolve_column(left_on)?;
    let rkey = right.resolve_column(right_on)?;

    // Right columns to carry over: every column except the key, order preserved.
    let right_cols: Vec<usize> = (0..right.width()).filter(|&i| i != rkey).collect();

    // Output headers: left headers, then the right table's non-key headers.
    let mut headers = left.headers.clone();
    for &i in &right_cols {
        headers.push(right.headers[i].clone());
    }

    // Index right rows by key value for O(1) lookup per left row.
    let mut index: HashMap<&str, Vec<&Vec<String>>> = HashMap::new();
    for row in &right.rows {
        index.entry(right.cell(row, rkey)).or_default().push(row);
    }

    let extra = right_cols.len();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for lrow in &left.rows {
        // Normalize the left portion to the full left width so ragged input
        // rows stay column-aligned with the appended right columns.
        let left_part: Vec<String> = (0..left.width())
            .map(|i| left.cell(lrow, i).to_string())
            .collect();

        match index.get(left.cell(lrow, lkey)) {
            Some(matches) => {
                for rrow in matches {
                    let mut out = left_part.clone();
                    out.extend(right_cols.iter().map(|&i| right.cell(rrow, i).to_string()));
                    rows.push(out);
                }
            }
            None if how == How::Left => {
                let mut out = left_part;
                out.extend(std::iter::repeat_n(String::new(), extra));
                rows.push(out);
            }
            None => {}
        }
    }

    Ok(Table::new(headers, rows))
}
