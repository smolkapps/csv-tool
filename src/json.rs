//! JSON conversion: `to-json` (table -> array of objects) and `from-json`
//! (array of objects -> table).
//!
//! `to-json` emits one object per row, keyed by header name, with every value a
//! JSON string (CSV has no types, so we don't guess). `from-json` accepts an
//! array of objects; the key set is the union of all objects' keys, ordered by
//! first appearance, so column order is stable and predictable.

use crate::table::Table;
use anyhow::{anyhow, Context, Result};
use serde_json::{Map, Value};

/// Serialize a table to a pretty-printed JSON array of objects.
pub fn to_json(table: &Table) -> Result<String> {
    let mut arr: Vec<Value> = Vec::with_capacity(table.rows.len());
    for row in &table.rows {
        let mut obj = Map::new();
        for (i, header) in table.headers.iter().enumerate() {
            let cell = table.cell(row, i).to_string();
            obj.insert(header.clone(), Value::String(cell));
        }
        arr.push(Value::Object(obj));
    }
    serde_json::to_string_pretty(&Value::Array(arr)).context("failed to serialize JSON")
}

/// Parse a JSON array of objects into a table.
///
/// Column order = order of first key appearance across the array. Scalar values
/// (string/number/bool/null) are stringified; null becomes an empty cell.
/// Nested arrays/objects are re-serialized as compact JSON so no data is lost.
pub fn from_json(input: &str) -> Result<Table> {
    let value: Value = serde_json::from_str(input).context("input is not valid JSON")?;
    let arr = match value {
        Value::Array(a) => a,
        _ => return Err(anyhow!("expected a top-level JSON array of objects")),
    };

    // Build the ordered union of keys.
    let mut headers: Vec<String> = Vec::new();
    for item in &arr {
        let obj = item
            .as_object()
            .ok_or_else(|| anyhow!("every array element must be a JSON object"))?;
        for key in obj.keys() {
            if !headers.contains(key) {
                headers.push(key.clone());
            }
        }
    }

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(arr.len());
    for item in &arr {
        let obj = item.as_object().unwrap(); // checked above
        let row = headers
            .iter()
            .map(|h| obj.get(h).map(value_to_cell).unwrap_or_default())
            .collect();
        rows.push(row);
    }

    Ok(Table::new(headers, rows))
}

/// Convert a JSON scalar to a CSV cell string. Strings pass through unquoted;
/// numbers/bools use their natural rendering; null is empty; compound values are
/// re-serialized compactly.
fn value_to_cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}
