//! Per-column statistics for the `stats` subcommand.
//!
//! A column is treated as *numeric* when every non-empty cell parses as a
//! number. Numeric columns report count/nulls/min/max/mean/sum; everything else
//! reports count/nulls/distinct. Output is itself a CSV table so it composes
//! with the rest of the tool.

use crate::table::Table;
use crate::value::parse_num;
use std::collections::HashSet;

/// Computed statistics for one column.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnStats {
    pub column: String,
    /// Non-empty cell count.
    pub count: usize,
    /// Empty (after trim) cell count.
    pub nulls: usize,
    pub numeric: bool,
    // Numeric-only fields (None for non-numeric columns).
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub sum: Option<f64>,
    // Non-numeric-only field.
    pub distinct: Option<usize>,
}

/// Compute stats for every column in the table.
pub fn compute(table: &Table) -> Vec<ColumnStats> {
    (0..table.width())
        .map(|idx| compute_column(table, idx))
        .collect()
}

fn compute_column(table: &Table, idx: usize) -> ColumnStats {
    let name = table.headers[idx].clone();
    let mut nulls = 0usize;
    let mut nums: Vec<f64> = Vec::new();
    let mut all_numeric = true;
    let mut distinct: HashSet<&str> = HashSet::new();
    let mut nonempty = 0usize;

    for row in &table.rows {
        let cell = table.cell(row, idx);
        let trimmed = cell.trim();
        if trimmed.is_empty() {
            nulls += 1;
            continue;
        }
        nonempty += 1;
        distinct.insert(trimmed);
        match parse_num(trimmed) {
            Some(n) => nums.push(n),
            None => all_numeric = false,
        }
    }

    // A column with zero non-empty cells is not meaningfully numeric.
    let numeric = all_numeric && nonempty > 0;

    if numeric {
        let sum: f64 = nums.iter().sum();
        let count = nums.len();
        let mean = if count > 0 { sum / count as f64 } else { 0.0 };
        let min = nums.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        ColumnStats {
            column: name,
            count: nonempty,
            nulls,
            numeric: true,
            min: Some(min),
            max: Some(max),
            mean: Some(mean),
            sum: Some(sum),
            distinct: None,
        }
    } else {
        ColumnStats {
            column: name,
            count: nonempty,
            nulls,
            numeric: false,
            min: None,
            max: None,
            mean: None,
            sum: None,
            distinct: Some(distinct.len()),
        }
    }
}

/// Render stats as a `Table` so it can be written like any other output.
///
/// Columns: column, type, count, nulls, min, max, mean, sum, distinct. Fields
/// that don't apply to a row are left empty.
pub fn to_table(stats: &[ColumnStats]) -> Table {
    let headers = vec![
        "column".into(),
        "type".into(),
        "count".into(),
        "nulls".into(),
        "min".into(),
        "max".into(),
        "mean".into(),
        "sum".into(),
        "distinct".into(),
    ];
    let rows = stats
        .iter()
        .map(|s| {
            vec![
                s.column.clone(),
                if s.numeric { "numeric" } else { "text" }.to_string(),
                s.count.to_string(),
                s.nulls.to_string(),
                fmt_opt(s.min),
                fmt_opt(s.max),
                fmt_opt(s.mean),
                fmt_opt(s.sum),
                s.distinct.map(|d| d.to_string()).unwrap_or_default(),
            ]
        })
        .collect();
    Table::new(headers, rows)
}

/// Format an optional float compactly: integers print without a trailing `.0`,
/// fractions print with up to 6 significant decimals trimmed of trailing zeros.
fn fmt_opt(v: Option<f64>) -> String {
    match v {
        None => String::new(),
        Some(x) => fmt_num(x),
    }
}

fn fmt_num(x: f64) -> String {
    if x == x.trunc() && x.is_finite() {
        // Whole number: no decimal point.
        format!("{}", x as i64)
    } else {
        // Trim trailing zeros from a fixed-precision rendering.
        let s = format!("{:.6}", x);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}
