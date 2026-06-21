//! Unit tests over the library API using small in-memory CSV.
//!
//! These exercise the core transforms directly (no process spawn): parse,
//! select, filter, stats, sort, uniq, and a to-json -> from-json round trip.

use csv_tool::io::{read_table, write_table, CsvOpts};
use csv_tool::{filter, json, stats, transform};

/// A small, well-known sample used across tests.
const SAMPLE: &str = "name,age,city\n\
Alice,30,NYC\n\
Bob,40,LA\n\
Carol,25,NYC\n\
Dave,35,SF\n";

fn parse_sample() -> csv_tool::Table {
    let t = read_table(SAMPLE.as_bytes(), CsvOpts::default()).expect("parse");
    assert_eq!(t.headers, vec!["name", "age", "city"]);
    assert_eq!(t.len(), 4);
    t
}

#[test]
fn parse_reads_headers_and_rows() {
    let t = parse_sample();
    assert_eq!(t.width(), 3);
    assert_eq!(t.rows[0], vec!["Alice", "30", "NYC"]);
    assert_eq!(t.rows[3], vec!["Dave", "35", "SF"]);
}

#[test]
fn parse_no_header_synthesizes_names() {
    let data = "a,b,c\n1,2,3\n";
    let opts = CsvOpts {
        delim: b',',
        has_header: false,
    };
    let t = read_table(data.as_bytes(), opts).expect("parse");
    assert_eq!(t.headers, vec!["col1", "col2", "col3"]);
    assert_eq!(t.len(), 2);
    assert_eq!(t.rows[0], vec!["a", "b", "c"]);
}

#[test]
fn select_returns_right_columns_by_name() {
    let t = parse_sample();
    let out = transform::select(&t, "name,city").expect("select");
    assert_eq!(out.headers, vec!["name", "city"]);
    assert_eq!(out.rows[0], vec!["Alice", "NYC"]);
    assert_eq!(out.rows[1], vec!["Bob", "LA"]);
}

#[test]
fn select_by_index_and_reorders() {
    let t = parse_sample();
    // index 2 = city, 0 = name
    let out = transform::select(&t, "2,0").expect("select");
    assert_eq!(out.headers, vec!["city", "name"]);
    assert_eq!(out.rows[2], vec!["NYC", "Carol"]);
}

#[test]
fn drop_removes_named_columns() {
    let t = parse_sample();
    let out = transform::drop(&t, "age").expect("drop");
    assert_eq!(out.headers, vec!["name", "city"]);
    assert_eq!(out.rows[1], vec!["Bob", "LA"]);
}

#[test]
fn filter_age_gt_30_keeps_right_rows() {
    let t = parse_sample();
    let pred = filter::Predicate::parse("age > 30").expect("parse pred");
    let out = filter::apply(&t, &pred).expect("apply");
    // Bob(40) and Dave(35) only; numeric compare so "40" > "30" not lexical.
    let names: Vec<&str> = out.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, vec!["Bob", "Dave"]);
}

#[test]
fn filter_numeric_ge_is_inclusive() {
    let t = parse_sample();
    let pred = filter::Predicate::parse("age >= 35").expect("parse");
    let out = filter::apply(&t, &pred).expect("apply");
    let names: Vec<&str> = out.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, vec!["Bob", "Dave"]);
}

#[test]
fn filter_string_equality() {
    let t = parse_sample();
    let pred = filter::Predicate::parse("city == NYC").expect("parse");
    let out = filter::apply(&t, &pred).expect("apply");
    let names: Vec<&str> = out.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, vec!["Alice", "Carol"]);
}

#[test]
fn filter_contains() {
    let t = parse_sample();
    let pred = filter::Predicate::parse("name contains a").expect("parse");
    let out = filter::apply(&t, &pred).expect("apply");
    // "Carol" and "Dave" contain lowercase 'a'; Alice/Bob do not.
    let names: Vec<&str> = out.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, vec!["Carol", "Dave"]);
}

#[test]
fn filter_not_equal() {
    let t = parse_sample();
    let pred = filter::Predicate::parse("city != NYC").expect("parse");
    let out = filter::apply(&t, &pred).expect("apply");
    let names: Vec<&str> = out.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, vec!["Bob", "Dave"]);
}

#[test]
fn stats_numeric_mean_sum_min_max() {
    let t = parse_sample();
    let s = stats::compute(&t);
    // Column 1 = age: 30,40,25,35
    let age = s.iter().find(|c| c.column == "age").expect("age stats");
    assert!(age.numeric);
    assert_eq!(age.count, 4);
    assert_eq!(age.nulls, 0);
    assert_eq!(age.sum, Some(130.0));
    assert_eq!(age.mean, Some(32.5));
    assert_eq!(age.min, Some(25.0));
    assert_eq!(age.max, Some(40.0));
}

#[test]
fn stats_text_count_distinct() {
    let t = parse_sample();
    let s = stats::compute(&t);
    let city = s.iter().find(|c| c.column == "city").expect("city stats");
    assert!(!city.numeric);
    assert_eq!(city.count, 4);
    // NYC, LA, SF distinct => 3
    assert_eq!(city.distinct, Some(3));
}

#[test]
fn stats_counts_nulls() {
    let data = "x,y\n1,a\n,b\n3,\n";
    let t = read_table(data.as_bytes(), CsvOpts::default()).expect("parse");
    let s = stats::compute(&t);
    let x = s.iter().find(|c| c.column == "x").unwrap();
    // x has one empty cell.
    assert_eq!(x.nulls, 1);
    assert_eq!(x.count, 2);
    // x is numeric (1, 3) ignoring the empty.
    assert!(x.numeric);
    assert_eq!(x.sum, Some(4.0));
}

#[test]
fn sort_numeric_ascending() {
    let t = parse_sample();
    let out = transform::sort(&t, "age", false, true).expect("sort");
    let ages: Vec<&str> = out.rows.iter().map(|r| r[1].as_str()).collect();
    assert_eq!(ages, vec!["25", "30", "35", "40"]);
}

#[test]
fn sort_numeric_descending() {
    let t = parse_sample();
    let out = transform::sort(&t, "age", true, true).expect("sort");
    let ages: Vec<&str> = out.rows.iter().map(|r| r[1].as_str()).collect();
    assert_eq!(ages, vec!["40", "35", "30", "25"]);
}

#[test]
fn sort_auto_numeric_without_flag() {
    // Even without --numeric, age cells are all numeric so auto rule kicks in.
    let t = parse_sample();
    let out = transform::sort(&t, "age", false, false).expect("sort");
    let ages: Vec<&str> = out.rows.iter().map(|r| r[1].as_str()).collect();
    assert_eq!(ages, vec!["25", "30", "35", "40"]);
}

#[test]
fn sort_string_lexical() {
    let t = parse_sample();
    let out = transform::sort(&t, "name", false, false).expect("sort");
    let names: Vec<&str> = out.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, vec!["Alice", "Bob", "Carol", "Dave"]);
}

#[test]
fn uniq_whole_row() {
    let data = "a,b\n1,x\n1,x\n2,y\n1,x\n";
    let t = read_table(data.as_bytes(), CsvOpts::default()).expect("parse");
    let out = transform::uniq(&t, None).expect("uniq");
    assert_eq!(out.len(), 2);
    assert_eq!(out.rows[0], vec!["1", "x"]);
    assert_eq!(out.rows[1], vec!["2", "y"]);
}

#[test]
fn uniq_by_column() {
    let data = "id,name\n1,Alice\n2,Bob\n1,Alice2\n3,Carol\n2,Bob2\n";
    let t = read_table(data.as_bytes(), CsvOpts::default()).expect("parse");
    let out = transform::uniq(&t, Some("id")).expect("uniq");
    // First occurrence of each id kept: 1,2,3
    let ids: Vec<&str> = out.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(ids, vec!["1", "2", "3"]);
    // And it keeps the FIRST row's other columns.
    assert_eq!(out.rows[0], vec!["1", "Alice"]);
}

#[test]
fn clean_trims_and_drops_empty_rows() {
    let data = "name , age\n  Alice ,  30 \n,\n Bob,40\n";
    let t = read_table(data.as_bytes(), CsvOpts::default()).expect("parse");
    let out = transform::clean(&t);
    // Header trimmed.
    assert_eq!(out.headers, vec!["name", "age"]);
    // Empty row dropped, others trimmed.
    assert_eq!(out.len(), 2);
    assert_eq!(out.rows[0], vec!["Alice", "30"]);
    assert_eq!(out.rows[1], vec!["Bob", "40"]);
}

#[test]
fn head_and_tail() {
    let t = parse_sample();
    let h = transform::head(&t, 2);
    assert_eq!(h.len(), 2);
    assert_eq!(h.rows[0][0], "Alice");
    assert_eq!(h.rows[1][0], "Bob");

    let tl = transform::tail(&t, 2);
    assert_eq!(tl.len(), 2);
    assert_eq!(tl.rows[0][0], "Carol");
    assert_eq!(tl.rows[1][0], "Dave");

    // n larger than len returns everything.
    assert_eq!(transform::head(&t, 100).len(), 4);
    assert_eq!(transform::tail(&t, 100).len(), 4);
}

#[test]
fn to_json_then_from_json_round_trip() {
    let t = parse_sample();
    let js = json::to_json(&t).expect("to_json");
    // Sanity: it's an array of objects with our keys.
    assert!(js.contains("\"name\""));
    assert!(js.contains("\"Alice\""));
    // Column ORDER must be preserved in the JSON: "name" key appears before
    // "age" before "city" (not alphabetized to age/city/name).
    let pos_name = js.find("\"name\"").unwrap();
    let pos_age = js.find("\"age\"").unwrap();
    let pos_city = js.find("\"city\"").unwrap();
    assert!(
        pos_name < pos_age && pos_age < pos_city,
        "JSON keys must keep column order"
    );

    let back = json::from_json(&js).expect("from_json");
    // Round trip preserves header order and all rows exactly.
    assert_eq!(back.headers, t.headers);
    assert_eq!(back.rows, t.rows);
}

#[test]
fn from_json_union_of_keys() {
    // Objects with differing keys: union, first-appearance order, missing -> "".
    let input = r#"[{"a":"1","b":"2"},{"a":"3","c":"4"}]"#;
    let t = json::from_json(input).expect("from_json");
    assert_eq!(t.headers, vec!["a", "b", "c"]);
    assert_eq!(t.rows[0], vec!["1", "2", ""]);
    assert_eq!(t.rows[1], vec!["3", "", "4"]);
}

#[test]
fn from_json_stringifies_scalars() {
    // Numbers, bools, null become strings / empty.
    let input = r#"[{"n":42,"b":true,"z":null}]"#;
    let t = json::from_json(input).expect("from_json");
    assert_eq!(t.headers, vec!["n", "b", "z"]);
    assert_eq!(t.rows[0], vec!["42", "true", ""]);
}

#[test]
fn write_table_round_trips_csv() {
    let t = parse_sample();
    let mut buf: Vec<u8> = Vec::new();
    write_table(&mut buf, &t, b',', true).expect("write");
    let s = String::from_utf8(buf).expect("utf8");
    // Re-parse and compare.
    let reparsed = read_table(s.as_bytes(), CsvOpts::default()).expect("reparse");
    assert_eq!(reparsed.headers, t.headers);
    assert_eq!(reparsed.rows, t.rows);
}

#[test]
fn filter_adjacent_operator_no_spaces() {
    // `age>30` with no spaces must parse the same as `age > 30`.
    let t = parse_sample();
    let pred = filter::Predicate::parse("age>30").expect("parse");
    let out = filter::apply(&t, &pred).expect("apply");
    let names: Vec<&str> = out.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, vec!["Bob", "Dave"]);
}

#[test]
fn filter_unknown_column_errors() {
    let t = parse_sample();
    let pred = filter::Predicate::parse("nope == 1").expect("parse");
    assert!(filter::apply(&t, &pred).is_err());
}
