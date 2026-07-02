//! csv-tool library core.
//!
//! The binary (`main.rs`) is a thin CLI shell over these modules. Keeping the
//! logic here makes every operation unit-testable without spawning a process.
//!
//! Module map:
//! - [`table`]: the `Table` model + column resolution.
//! - [`value`]: numeric-vs-string comparison rules shared by filter & sort.
//! - [`io`]: CSV read/write.
//! - [`filter`]: predicate parsing/evaluation.
//! - [`transform`]: select/drop/head/tail/sort/uniq/clean.
//! - [`stats`]: per-column statistics.
//! - [`join`]: join two tables on a key column.
//! - [`json`]: to-json / from-json.

pub mod filter;
pub mod io;
pub mod join;
pub mod json;
pub mod stats;
pub mod table;
pub mod transform;
pub mod value;

pub use filter::{Op, Predicate};
pub use io::{read_table, write_table, CsvOpts};
pub use join::{join, How};
pub use stats::{compute as compute_stats, ColumnStats};
pub use table::Table;
