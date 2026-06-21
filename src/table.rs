//! Core in-memory table model: headers + string rows.
//!
//! A `Table` is the unit most operations work on. Parsing reads CSV into a
//! `Table`; transforms (select, filter, sort, uniq, clean) return new `Table`s;
//! writers serialize a `Table` back to CSV or JSON.

use anyhow::{anyhow, Result};

/// A simple tabular structure: a header row plus data rows of equal-ish width.
///
/// Rows are stored as owned `String`s. We deliberately keep everything as text
/// (CSV is text) and only interpret numbers at comparison/stat time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Table {
    /// Column names. When the source has no header, these are synthesized as
    /// `col1`, `col2`, ... so name-based selection still works.
    pub headers: Vec<String>,
    /// Data rows. Each inner `Vec<String>` is one record.
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// Build a table directly from parts. Used by tests and transforms.
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Table { headers, rows }
    }

    /// Number of columns, derived from the header.
    pub fn width(&self) -> usize {
        self.headers.len()
    }

    /// Number of data rows (excludes the header).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Resolve a single column specifier to a column index.
    ///
    /// A specifier may be a column *name* (exact match against `headers`) or a
    /// 0-based *index* given as a decimal string. Names take precedence: if a
    /// header literally is `"0"`, that name wins over index 0. This matters for
    /// real-world CSVs with numeric-looking headers.
    pub fn resolve_column(&self, spec: &str) -> Result<usize> {
        let spec = spec.trim();
        if let Some(idx) = self.headers.iter().position(|h| h == spec) {
            return Ok(idx);
        }
        // Fall back to numeric index.
        if let Ok(idx) = spec.parse::<usize>() {
            if idx < self.headers.len() {
                return Ok(idx);
            }
            return Err(anyhow!(
                "column index {} out of range (table has {} columns)",
                idx,
                self.headers.len()
            ));
        }
        Err(anyhow!("no such column: '{}'", spec))
    }

    /// Resolve a comma-separated list of column specifiers to indices,
    /// preserving the order the user gave (so `select c,a,b` reorders).
    pub fn resolve_columns(&self, specs: &str) -> Result<Vec<usize>> {
        let mut out = Vec::new();
        for part in specs.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            out.push(self.resolve_column(part)?);
        }
        if out.is_empty() {
            return Err(anyhow!("no columns specified"));
        }
        Ok(out)
    }

    /// Safely fetch a cell, returning an empty string for short rows. CSV in the
    /// wild is ragged; treating missing trailing cells as empty avoids panics.
    pub fn cell<'a>(&'a self, row: &'a [String], idx: usize) -> &'a str {
        row.get(idx).map(|s| s.as_str()).unwrap_or("")
    }
}
