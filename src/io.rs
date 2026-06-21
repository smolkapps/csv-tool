//! CSV reading and writing.
//!
//! Reading slurps the whole stream into a `Table`. True streaming row-by-row is
//! only possible for the trivially-local transforms (head/tail/clean); ops like
//! sort, stats, and uniq are inherently buffering, so for a uniform API we
//! materialize. Records are read with a flexible reader so ragged rows don't
//! abort the parse.

use crate::table::Table;
use anyhow::{Context, Result};
use std::io::{Read, Write};

/// Options controlling how CSV is parsed and rendered.
#[derive(Debug, Clone, Copy)]
pub struct CsvOpts {
    /// Field delimiter (default `,`).
    pub delim: u8,
    /// When false, the first record is the header. When true, the data has no
    /// header and we synthesize `col1..colN`.
    pub has_header: bool,
}

impl Default for CsvOpts {
    fn default() -> Self {
        CsvOpts {
            delim: b',',
            has_header: true,
        }
    }
}

/// Read a full CSV stream into a `Table`.
///
/// `flexible(true)` lets rows have differing field counts without erroring —
/// real CSVs are messy. When `has_header` is false we read the first record as
/// data and name columns `col1..colN` based on the widest row seen so far.
pub fn read_table<R: Read>(reader: R, opts: CsvOpts) -> Result<Table> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(opts.delim)
        .has_headers(opts.has_header)
        .flexible(true)
        .from_reader(reader);

    let mut headers: Vec<String> = Vec::new();
    if opts.has_header {
        let hdr = rdr
            .headers()
            .context("failed to read CSV header row")?
            .clone();
        headers = hdr.iter().map(|s| s.to_string()).collect();
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.context("failed to parse a CSV record")?;
        let row: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        rows.push(row);
    }

    // Synthesize headers for headerless input, sized to the widest row.
    if !opts.has_header {
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        headers = (1..=width).map(|i| format!("col{}", i)).collect();
    }

    Ok(Table::new(headers, rows))
}

/// Write a `Table` to a writer as CSV. The header is emitted only when
/// `write_header` is true (callers pass `opts.has_header` so `--no-header` input
/// round-trips to `--no-header` output).
pub fn write_table<W: Write>(
    writer: W,
    table: &Table,
    delim: u8,
    write_header: bool,
) -> Result<()> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(writer);

    if write_header {
        wtr.write_record(&table.headers)
            .context("failed to write header")?;
    }
    for row in &table.rows {
        wtr.write_record(row).context("failed to write row")?;
    }
    wtr.flush().context("failed to flush CSV writer")?;
    Ok(())
}
