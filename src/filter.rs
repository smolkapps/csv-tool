//! Single-column predicate parsing and evaluation for the `filter` subcommand.
//!
//! Grammar (one predicate, one column):
//!     <col> <OP> <value>
//! where OP is one of: == != > >= < <= contains
//!
//! The column name may contain spaces, and the value may too, so we don't just
//! split on whitespace. Instead we find the operator token (longest-match,
//! surrounded by whitespace OR directly adjacent) and split there.

use crate::table::Table;
use crate::value::{compare_cells, parse_num};
use anyhow::{anyhow, Result};
use std::cmp::Ordering;

/// Comparison operators supported by `filter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
}

/// A parsed predicate: which column, which operator, and the literal value.
#[derive(Debug, Clone)]
pub struct Predicate {
    pub column: String,
    pub op: Op,
    pub value: String,
}

impl Predicate {
    /// Parse an expression like `age >= 30` or `name contains smith`.
    ///
    /// We scan for operator tokens in order of *decreasing length* so `>=`
    /// isn't mis-split as `>`. The word operator `contains` must be bounded by
    /// whitespace; symbolic operators may be adjacent to operands (`age>30`).
    pub fn parse(expr: &str) -> Result<Predicate> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Err(anyhow!("empty filter expression"));
        }

        // `contains` first: it's a word, must be space-delimited.
        if let Some(pos) = find_word_op(expr, "contains") {
            let column = expr[..pos].trim().to_string();
            let value = expr[pos + "contains".len()..].trim().to_string();
            return finish(column, Op::Contains, value);
        }

        // Symbolic operators, longest first so `>=`/`<=`/`==`/`!=` beat `>`/`<`.
        for (tok, op) in [
            (">=", Op::Ge),
            ("<=", Op::Le),
            ("==", Op::Eq),
            ("!=", Op::Ne),
            (">", Op::Gt),
            ("<", Op::Lt),
        ] {
            if let Some(pos) = expr.find(tok) {
                let column = expr[..pos].trim().to_string();
                let value = expr[pos + tok.len()..].trim().to_string();
                return finish(column, op, value);
            }
        }

        Err(anyhow!(
            "could not find an operator in filter '{}'; expected one of == != > >= < <= contains",
            expr
        ))
    }

    /// Evaluate this predicate against a single cell value.
    pub fn matches(&self, cell: &str) -> bool {
        match self.op {
            Op::Contains => cell.contains(&self.value),
            Op::Eq | Op::Ne => {
                // Equality uses numeric comparison when both sides are numbers so
                // `3.0 == 3` is true; otherwise exact string match.
                let equal = match (parse_num(cell), parse_num(&self.value)) {
                    (Some(a), Some(b)) => a == b,
                    _ => cell == self.value,
                };
                if self.op == Op::Eq {
                    equal
                } else {
                    !equal
                }
            }
            Op::Gt | Op::Ge | Op::Lt | Op::Le => {
                let ord = compare_cells(cell, &self.value);
                match self.op {
                    Op::Gt => ord == Ordering::Greater,
                    Op::Ge => ord != Ordering::Less,
                    Op::Lt => ord == Ordering::Less,
                    Op::Le => ord != Ordering::Greater,
                    _ => unreachable!(),
                }
            }
        }
    }
}

/// Apply a predicate to a table, returning a new table with matching rows.
pub fn apply(table: &Table, pred: &Predicate) -> Result<Table> {
    let idx = table.resolve_column(&pred.column)?;
    let rows = table
        .rows
        .iter()
        .filter(|row| pred.matches(table.cell(row, idx)))
        .cloned()
        .collect();
    Ok(Table::new(table.headers.clone(), rows))
}

/// Reject empty column/value early so we give a clean error instead of e.g.
/// matching against an empty literal.
fn finish(column: String, op: Op, value: String) -> Result<Predicate> {
    if column.is_empty() {
        return Err(anyhow!("filter is missing a column name on the left side"));
    }
    Ok(Predicate { column, op, value })
}

/// Find a whitespace-delimited word operator inside `expr`, returning its byte
/// offset. Requires a space (or string boundary) on both sides so a column
/// literally named `contains_x` isn't mistaken for the operator.
fn find_word_op(expr: &str, word: &str) -> Option<usize> {
    let bytes = expr.as_bytes();
    let wlen = word.len();
    let mut search_from = 0;
    while let Some(rel) = expr[search_from..].find(word) {
        let pos = search_from + rel;
        let before_ok = pos == 0 || bytes[pos - 1].is_ascii_whitespace();
        let after = pos + wlen;
        let after_ok = after >= expr.len() || bytes[after].is_ascii_whitespace();
        if before_ok && after_ok {
            return Some(pos);
        }
        search_from = pos + wlen;
    }
    None
}
