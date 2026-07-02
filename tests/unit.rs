//! Unit tests over the library API using small in-memory CSV.
//!
//! These exercise the core transforms directly (no process spawn): parse,
//! select, filter, stats, sort, uniq, and a to-json -> from-json round trip.

use csv_tool::io::{read_table, write_table, CsvOpts};
use csv_tool::join::How;
use csv_tool::{filter, frequency, join, json, stats, transform};

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

fn parse(data: &str) -> csv_tool::Table {
    read_table(data.as_bytes(), CsvOpts::default()).expect("parse")
}

#[test]
fn join_inner_preserves_column_order_and_drops_right_key() {
    let left = parse_sample(); // name,age,city
    let right = parse("city,country\nNYC,USA\nLA,USA\nSF,USA\n");
    let out = join::join(&left, &right, "city", "city", How::Inner).expect("join");
    // Left columns in order, then right's non-key columns: name,age,city,country.
    assert_eq!(out.headers, vec!["name", "age", "city", "country"]);
    assert_eq!(out.len(), 4);
    // Alice (NYC) picks up USA; key column appears once.
    let alice = out.rows.iter().find(|r| r[0] == "Alice").unwrap();
    assert_eq!(alice, &vec!["Alice", "30", "NYC", "USA"]);
}

#[test]
fn join_inner_drops_unmatched_rows() {
    let left = parse_sample();
    // Only NYC has a match; LA/SF rows on the left are dropped by an inner join.
    let right = parse("city,country\nNYC,USA\n");
    let out = join::join(&left, &right, "city", "city", How::Inner).expect("join");
    let names: Vec<&str> = out.rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, vec!["Alice", "Carol"]);
}

#[test]
fn join_left_keeps_unmatched_with_empty_right_cells() {
    let left = parse_sample();
    let right = parse("city,country\nNYC,USA\n");
    let out = join::join(&left, &right, "city", "city", How::Left).expect("join");
    // Every left row survives.
    assert_eq!(out.len(), 4);
    let bob = out.rows.iter().find(|r| r[0] == "Bob").unwrap();
    // Bob's city (LA) has no match -> country is empty.
    assert_eq!(bob, &vec!["Bob", "40", "LA", ""]);
}

#[test]
fn join_duplicate_keys_produce_cartesian_product() {
    let left = parse("id,name\n1,Alice\n2,Bob\n");
    // id 1 appears twice on the right: Alice should be emitted twice.
    let right = parse("id,role\n1,admin\n1,user\n2,guest\n");
    let out = join::join(&left, &right, "id", "id", How::Inner).expect("join");
    assert_eq!(out.headers, vec!["id", "name", "role"]);
    let roles: Vec<&str> = out
        .rows
        .iter()
        .filter(|r| r[1] == "Alice")
        .map(|r| r[2].as_str())
        .collect();
    assert_eq!(roles, vec!["admin", "user"]);
    assert_eq!(out.len(), 3);
}

#[test]
fn join_different_key_column_names() {
    let left = parse("uid,name\n1,Alice\n2,Bob\n");
    let right = parse("id,country\n1,USA\n2,UK\n");
    let out = join::join(&left, &right, "uid", "id", How::Inner).expect("join");
    // Left key kept (uid), right key (id) dropped.
    assert_eq!(out.headers, vec!["uid", "name", "country"]);
    assert_eq!(out.rows[0], vec!["1", "Alice", "USA"]);
    assert_eq!(out.rows[1], vec!["2", "Bob", "UK"]);
}

#[test]
fn join_unknown_key_errors() {
    let left = parse_sample();
    let right = parse("city,country\nNYC,USA\n");
    assert!(join::join(&left, &right, "nope", "city", How::Inner).is_err());
}

#[test]
fn frequency_counts_and_orders_by_descending_count() {
    // city: NYC x2, LA x1, SF x1 -> NYC first, then LA/SF (tie broken by value).
    let t = parse_sample();
    let out = frequency::frequency(&t, "city").expect("frequency");
    assert_eq!(out.headers, vec!["value", "count"]);
    assert_eq!(out.rows[0], vec!["NYC", "2"]);
    // Ties (count 1) ordered by value ascending: LA before SF.
    assert_eq!(out.rows[1], vec!["LA", "1"]);
    assert_eq!(out.rows[2], vec!["SF", "1"]);
    assert_eq!(out.len(), 3);
}

#[test]
fn frequency_by_index_and_counts_empty_values() {
    // Empty cells are counted as an empty-string value.
    let data = "k,v\na,1\n,2\nb,3\na,4\n";
    let t = parse(data);
    let out = frequency::frequency(&t, "0").expect("frequency");
    // a x2, then "" and b (count 1) tie-broken by value: "" < "b".
    assert_eq!(out.rows[0], vec!["a", "2"]);
    assert_eq!(out.rows[1], vec!["", "1"]);
    assert_eq!(out.rows[2], vec!["b", "1"]);
    assert_eq!(out.len(), 3);
}

#[test]
fn frequency_unknown_column_errors() {
    let t = parse_sample();
    assert!(frequency::frequency(&t, "nope").is_err());
}
