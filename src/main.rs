//! csv-tool — a fast CSV swiss-army knife.
//!
//! Reads CSV from a file (or stdin) and writes to a file (`-o`) or stdout.
//! Subcommands are thin wrappers around the `csv_tool` library.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use csv_tool::{filter, io, json, stats, transform, Table};
use std::fs::File;
use std::io::{self as stdio, BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

/// A fast CSV swiss-army knife: slice, filter, summarize and convert CSV.
#[derive(Parser, Debug)]
#[command(name = "csv-tool", version, about, long_about = None)]
struct Cli {
    /// Input CSV file. Reads stdin when omitted or given as `-`.
    #[arg(short = 'i', long = "input", global = true)]
    input: Option<PathBuf>,

    /// Output file. Writes stdout when omitted.
    #[arg(short = 'o', long = "output", global = true)]
    output: Option<PathBuf>,

    /// Field delimiter (single character). Defaults to `,`.
    #[arg(long = "delim", global = true)]
    delim: Option<char>,

    /// Treat input as having no header row (columns become col1..colN).
    #[arg(long = "no-header", global = true, default_value_t = false)]
    no_header: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print the first N rows.
    Head {
        #[arg(short = 'n', long = "lines", default_value_t = 10)]
        n: usize,
    },
    /// Print the last N rows.
    Tail {
        #[arg(short = 'n', long = "lines", default_value_t = 10)]
        n: usize,
    },
    /// Keep only the given columns (by name or 0-based index), comma-separated.
    Select {
        /// e.g. `name,age` or `0,2,1`.
        cols: String,
    },
    /// Drop the given columns (by name or 0-based index), comma-separated.
    Drop { cols: String },
    /// Keep rows matching a single-column predicate, e.g. `age >= 30`.
    Filter {
        /// `<col> <OP> <value>`; OP in == != > >= < <= contains.
        expr: String,
    },
    /// Per-column statistics (numeric: count/nulls/min/max/mean/sum; text:
    /// count/nulls/distinct).
    Stats,
    /// Sort rows by a column.
    Sort {
        #[arg(long = "by")]
        by: String,
        /// Sort descending.
        #[arg(long = "desc", default_value_t = false)]
        desc: bool,
        /// Force numeric comparison.
        #[arg(long = "numeric", default_value_t = false)]
        numeric: bool,
    },
    /// Remove duplicate rows (optionally keyed on a subset of columns).
    Uniq {
        /// Columns to dedupe on (comma-separated). Whole-row when omitted.
        #[arg(long = "by")]
        by: Option<String>,
    },
    /// Convert CSV to a JSON array of objects.
    #[command(name = "to-json")]
    ToJson,
    /// Convert a JSON array of objects to CSV.
    #[command(name = "from-json")]
    FromJson,
    /// List column names with their 0-based indices.
    Headers,
    /// Trim whitespace, drop fully-empty rows, normalize line endings.
    Clean,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // anyhow's chain gives a useful "caused by" trail on stderr.
            eprintln!("csv-tool: error: {:#}", e);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let delim = resolve_delim(cli.delim)?;
    let opts = io::CsvOpts {
        delim,
        has_header: !cli.no_header,
    };

    // `from-json` is special: its input is JSON, not CSV. Handle it before the
    // generic CSV read path.
    if let Command::FromJson = cli.command {
        let input = read_input_string(&cli.input)?;
        let table = json::from_json(&input)?;
        return write_csv_out(&cli.output, &table, delim, opts.has_header);
    }

    // Every other subcommand starts from a parsed CSV table.
    let table = read_csv_in(&cli.input, opts).context("failed to read input CSV")?;

    match cli.command {
        Command::Head { n } => {
            let out = transform::head(&table, n);
            write_csv_out(&cli.output, &out, delim, opts.has_header)
        }
        Command::Tail { n } => {
            let out = transform::tail(&table, n);
            write_csv_out(&cli.output, &out, delim, opts.has_header)
        }
        Command::Select { cols } => {
            let out = transform::select(&table, &cols)?;
            write_csv_out(&cli.output, &out, delim, opts.has_header)
        }
        Command::Drop { cols } => {
            let out = transform::drop(&table, &cols)?;
            write_csv_out(&cli.output, &out, delim, opts.has_header)
        }
        Command::Filter { expr } => {
            let pred = filter::Predicate::parse(&expr)?;
            let out = filter::apply(&table, &pred)?;
            write_csv_out(&cli.output, &out, delim, opts.has_header)
        }
        Command::Stats => {
            let s = stats::compute(&table);
            let out = stats::to_table(&s);
            // Stats output always has its own header row regardless of input.
            write_csv_out(&cli.output, &out, delim, true)
        }
        Command::Sort { by, desc, numeric } => {
            let out = transform::sort(&table, &by, desc, numeric)?;
            write_csv_out(&cli.output, &out, delim, opts.has_header)
        }
        Command::Uniq { by } => {
            let out = transform::uniq(&table, by.as_deref())?;
            write_csv_out(&cli.output, &out, delim, opts.has_header)
        }
        Command::ToJson => {
            let s = json::to_json(&table)?;
            write_text_out(&cli.output, &s)
        }
        Command::Headers => {
            // List "index,name" pairs as a tiny CSV with its own header.
            let headers = vec!["index".to_string(), "name".to_string()];
            let rows = table
                .headers
                .iter()
                .enumerate()
                .map(|(i, h)| vec![i.to_string(), h.clone()])
                .collect();
            let out = Table::new(headers, rows);
            write_csv_out(&cli.output, &out, delim, true)
        }
        Command::Clean => {
            let out = transform::clean(&table);
            write_csv_out(&cli.output, &out, delim, opts.has_header)
        }
        Command::FromJson => unreachable!("handled above"),
    }
}

/// Validate and convert the delimiter char to a single byte. We reject
/// multi-byte (non-ASCII) delimiters because the `csv` crate's delimiter is one
/// byte; this gives a clear error instead of a silent truncation.
fn resolve_delim(d: Option<char>) -> Result<u8> {
    match d {
        None => Ok(b','),
        Some(c) => {
            if c.is_ascii() {
                Ok(c as u8)
            } else {
                Err(anyhow!(
                    "delimiter must be a single ASCII character, got '{}'",
                    c
                ))
            }
        }
    }
}

/// Open the input as a buffered reader: a file when a path (other than `-`) is
/// given, otherwise stdin.
fn open_input(path: &Option<PathBuf>) -> Result<Box<dyn Read>> {
    match path {
        Some(p) if p.as_os_str() != "-" => {
            let f = File::open(p)
                .with_context(|| format!("could not open input file {}", p.display()))?;
            Ok(Box::new(BufReader::new(f)))
        }
        _ => Ok(Box::new(BufReader::new(stdio::stdin()))),
    }
}

fn read_csv_in(path: &Option<PathBuf>, opts: io::CsvOpts) -> Result<Table> {
    let reader = open_input(path)?;
    io::read_table(reader, opts)
}

fn read_input_string(path: &Option<PathBuf>) -> Result<String> {
    let mut reader = open_input(path)?;
    let mut buf = String::new();
    reader
        .read_to_string(&mut buf)
        .context("failed to read input")?;
    Ok(buf)
}

/// Open the output as a buffered writer: a file when `-o` is given, else stdout.
fn open_output(path: &Option<PathBuf>) -> Result<Box<dyn Write>> {
    match path {
        Some(p) => {
            let f = File::create(p)
                .with_context(|| format!("could not create output file {}", p.display()))?;
            Ok(Box::new(BufWriter::new(f)))
        }
        None => Ok(Box::new(BufWriter::new(stdio::stdout()))),
    }
}

fn write_csv_out(
    path: &Option<PathBuf>,
    table: &Table,
    delim: u8,
    write_header: bool,
) -> Result<()> {
    let writer = open_output(path)?;
    io::write_table(writer, table, delim, write_header)
}

fn write_text_out(path: &Option<PathBuf>, text: &str) -> Result<()> {
    let mut writer = open_output(path)?;
    writer
        .write_all(text.as_bytes())
        .context("failed to write output")?;
    // to-json output is a single JSON document; end with a newline for tidiness.
    if !text.ends_with('\n') {
        writer.write_all(b"\n").ok();
    }
    writer.flush().context("failed to flush output")?;
    Ok(())
}
