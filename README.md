# csv-tool

A fast CSV swiss-army knife for the command line. Slice, filter, summarize, sort,
deduplicate, and convert CSV — reading from a file or stdin and writing to a file
or stdout, so it drops cleanly into shell pipelines.

Built in Rust on top of [`clap`](https://crates.io/crates/clap),
[`csv`](https://crates.io/crates/csv), [`serde_json`](https://crates.io/crates/serde_json),
and [`anyhow`](https://crates.io/crates/anyhow).

## Install

```sh
cargo build --release
# binary at target/release/csv-tool
```

## Usage

```
csv-tool [GLOBAL FLAGS] <SUBCOMMAND> [ARGS]
```

Input is read from `-i/--input <FILE>` or stdin (when omitted or `-`). Output goes
to `-o/--output <FILE>` or stdout.

### Global flags

| Flag | Meaning |
|------|---------|
| `-i, --input <FILE>` | Read CSV from a file (default: stdin). |
| `-o, --output <FILE>` | Write to a file (default: stdout). |
| `--delim <CHAR>` | Field delimiter (default `,`). |
| `--no-header` | Input has no header row; columns become `col1..colN`. |

### Subcommands

| Command | Description |
|---------|-------------|
| `head -n N` | First N rows (default 10). |
| `tail -n N` | Last N rows (default 10). |
| `select <cols>` | Keep only these columns (name or 0-based index, comma list). Reorders. |
| `drop <cols>` | Drop these columns. |
| `filter <expr>` | Keep rows matching `col OP value` where OP is `== != > >= < <= contains`. Numeric compare when both sides are numbers, else string. |
| `stats` | Per-column summary. Numeric: count, nulls, min, max, mean, sum. Text: count, nulls, distinct. |
| `frequency <col>` | Count occurrences of each distinct value in a column (`value,count`), ordered by descending count. |
| `sort --by <col> [--desc] [--numeric]` | Sort rows by a column. |
| `join <right.csv> --on <col> [--left-on <c>] [--right-on <c>] [--left]` | Join a second CSV on a key column. |
| `uniq [--by <cols>]` | Remove duplicate rows (whole-row, or keyed on a subset). |
| `to-json` | Emit a JSON array of objects. |
| `from-json` | Read a JSON array of objects and emit CSV. |
| `headers` | List column names with their 0-based indices. |
| `clean` | Trim whitespace, drop fully-empty rows, normalize line endings. |

## Examples

```sh
# Top 5 rows of a file
csv-tool -i data.csv head -n 5

# Pick columns and filter, piping through stdin
cat data.csv | csv-tool select name,age | csv-tool filter 'age >= 30'

# Summary statistics
csv-tool -i data.csv stats

# Value counts: how many rows share each city, most common first
csv-tool -i data.csv frequency city

# Sort numerically, descending
csv-tool -i data.csv sort --by age --numeric --desc

# Join people.csv with cities.csv on the shared `city` column
csv-tool -i people.csv join cities.csv --on city

# Semicolon-delimited, no header: filter column index 1
csv-tool --no-header --delim ';' filter '1 > 30' < data.csv

# CSV <-> JSON
csv-tool -i data.csv to-json > data.json
csv-tool -i data.json from-json > roundtrip.csv
```

## Notes on semantics

- **Numeric vs string comparison** (filter & sort): when *both* operands parse as
  numbers, the comparison is numeric; otherwise it's lexical. `sort --numeric`
  forces numeric ordering (non-numbers sort first).
- **Column references** accept either a header name or a 0-based index. Names win
  when ambiguous (a header literally named `0` beats index 0).
- **Ragged rows** (varying field counts) are tolerated; missing trailing cells
  read as empty.
- **`join`** matches rows by key value and outputs all of the left table's
  columns followed by the right table's columns *except* its key (so the key
  appears once and both tables' column order is preserved). Duplicate keys
  produce the cartesian product per key, like a SQL join. It's an inner join by
  default; `--left` keeps unmatched left rows with empty right cells. Use
  `--left-on`/`--right-on` when the two files name the key column differently.
- **`--no-header` round-trips**: headerless input produces headerless output.
- Errors (bad expressions, unknown columns, missing files, malformed JSON) print a
  clear message to stderr and exit non-zero.

## Tests

```sh
cargo test
```

Unit tests cover the library transforms over a small in-memory CSV; integration
tests spawn the real binary and drive it via stdin (including `--no-header`,
`--delim ';'`, and malformed/empty input).

## License

MIT — see [LICENSE](LICENSE).
