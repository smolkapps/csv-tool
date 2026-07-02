//! Integration tests that spawn the real `csv-tool` binary.
//!
//! These drive the actual trigger path: stdin -> process -> stdout, exercising
//! arg parsing, the delimiter/no-header flags, and exit codes on bad input —
//! the parts unit tests over the library can't reach.

use assert_cmd::Command;
use predicates::prelude::*;

const SAMPLE: &str = "name,age,city\n\
Alice,30,NYC\n\
Bob,40,LA\n\
Carol,25,NYC\n\
Dave,35,SF\n";

/// Helper to get a fresh Command for our binary.
fn cmd() -> Command {
    Command::cargo_bin("csv-tool").expect("binary builds")
}

#[test]
fn head_via_stdin() {
    cmd()
        .args(["head", "-n", "2"])
        .write_stdin(SAMPLE)
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("Bob"))
        .stdout(predicate::str::contains("Carol").not());
}

#[test]
fn select_via_stdin() {
    cmd()
        .args(["select", "name,city"])
        .write_stdin(SAMPLE)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("name,city"))
        .stdout(predicate::str::contains("Alice,NYC"))
        // age column should be gone.
        .stdout(predicate::str::contains("30").not());
}

#[test]
fn filter_age_gt_30_via_stdin() {
    cmd()
        .args(["filter", "age > 30"])
        .write_stdin(SAMPLE)
        .assert()
        .success()
        .stdout(predicate::str::contains("Bob,40"))
        .stdout(predicate::str::contains("Dave,35"))
        .stdout(predicate::str::contains("Alice").not())
        .stdout(predicate::str::contains("Carol").not());
}

#[test]
fn stats_via_stdin() {
    cmd()
        .arg("stats")
        .write_stdin(SAMPLE)
        .assert()
        .success()
        // Header of the stats table.
        .stdout(predicate::str::contains(
            "column,type,count,nulls,min,max,mean,sum,distinct",
        ))
        // age row: numeric, sum 130, mean 32.5
        .stdout(predicate::str::contains("age,numeric,4,0,25,40,32.5,130"))
        // city row: text, distinct 3
        .stdout(predicate::str::contains("city,text,4,0,,,,,3"));
}

#[test]
fn sort_numeric_desc_via_stdin() {
    let out = cmd()
        .args(["sort", "--by", "age", "--numeric", "--desc"])
        .write_stdin(SAMPLE)
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // First data line should be Bob (40), last Carol (25).
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "name,age,city");
    assert!(lines[1].starts_with("Bob,40"));
    assert!(lines[4].starts_with("Carol,25"));
}

#[test]
fn headers_lists_indices() {
    cmd()
        .arg("headers")
        .write_stdin(SAMPLE)
        .assert()
        .success()
        .stdout(predicate::str::contains("index,name"))
        .stdout(predicate::str::contains("0,name"))
        .stdout(predicate::str::contains("1,age"))
        .stdout(predicate::str::contains("2,city"));
}

#[test]
fn no_header_with_semicolon_delim() {
    // Headerless, semicolon-delimited input. Select col2 (0-based index 1).
    let data = "Alice;30;NYC\nBob;40;LA\n";
    cmd()
        .args(["--no-header", "--delim", ";", "select", "1"])
        .write_stdin(data)
        .assert()
        .success()
        // Output should also be semicolon-delimited and headerless.
        .stdout(predicate::str::contains("30"))
        .stdout(predicate::str::contains("40"))
        // No synthesized header should appear in output.
        .stdout(predicate::str::contains("col1").not());
}

#[test]
fn no_header_filter_by_index() {
    // Filter headerless data on column index 1 > 30.
    let data = "Alice;30;NYC\nBob;40;LA\nCarol;25;NYC\n";
    cmd()
        .args(["--no-header", "--delim", ";", "filter", "1 > 30"])
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("Bob;40;LA"))
        .stdout(predicate::str::contains("Alice").not())
        .stdout(predicate::str::contains("Carol").not());
}

#[test]
fn semicolon_delim_with_header() {
    let data = "name;age\nAlice;30\nBob;40\n";
    cmd()
        .args(["--delim", ";", "filter", "age >= 40"])
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("Bob;40"))
        .stdout(predicate::str::contains("Alice").not());
}

#[test]
fn to_json_and_from_json_round_trip_via_pipe() {
    // to-json output fed back into from-json should reproduce the CSV.
    let json_out = cmd().arg("to-json").write_stdin(SAMPLE).assert().success();
    let json_text = String::from_utf8(json_out.get_output().stdout.clone()).unwrap();
    assert!(json_text.contains("\"Alice\""));

    cmd()
        .arg("from-json")
        .write_stdin(json_text)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("name,age,city"))
        .stdout(predicate::str::contains("Alice,30,NYC"))
        .stdout(predicate::str::contains("Dave,35,SF"));
}

#[test]
fn uniq_via_stdin() {
    let data = "a,b\n1,x\n1,x\n2,y\n";
    cmd()
        .arg("uniq")
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("1,x"))
        .stdout(predicate::str::contains("2,y"));
}

#[test]
fn clean_drops_empty_rows_via_stdin() {
    let data = "name , age\n  Alice ,  30 \n,\n Bob,40\n";
    cmd()
        .arg("clean")
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice,30"))
        .stdout(predicate::str::contains("Bob,40"));
}

#[test]
fn join_on_city_via_stdin() {
    // Right-hand table lives in a file; left comes from stdin.
    let dir = std::env::temp_dir();
    let right = dir.join("csv_tool_join_right.csv");
    std::fs::write(&right, "city,country\nNYC,USA\nLA,USA\nSF,USA\n").expect("write right file");

    cmd()
        .args(["join", right.to_str().unwrap(), "--on", "city"])
        .write_stdin(SAMPLE)
        .assert()
        .success()
        // Column order preserved; right key column not duplicated.
        .stdout(predicate::str::starts_with("name,age,city,country"))
        .stdout(predicate::str::contains("Alice,30,NYC,USA"))
        .stdout(predicate::str::contains("Bob,40,LA,USA"));

    std::fs::remove_file(&right).ok();
}

#[test]
fn join_missing_key_errors() {
    let dir = std::env::temp_dir();
    let right = dir.join("csv_tool_join_nokey.csv");
    std::fs::write(&right, "city,country\nNYC,USA\n").expect("write right file");

    // No --on / --left-on / --right-on given: clear error, non-zero exit.
    cmd()
        .args(["join", right.to_str().unwrap()])
        .write_stdin(SAMPLE)
        .assert()
        .failure()
        .stderr(predicate::str::contains("key column"));

    std::fs::remove_file(&right).ok();
}

// --- Error / edge-case handling -----------------------------------------

#[test]
fn filter_bad_expression_errors() {
    // No operator -> non-zero exit and a clear message on stderr.
    cmd()
        .args(["filter", "agewithnooperator"])
        .write_stdin(SAMPLE)
        .assert()
        .failure()
        .stderr(predicate::str::contains("operator"));
}

#[test]
fn select_unknown_column_errors() {
    cmd()
        .args(["select", "nonexistent"])
        .write_stdin(SAMPLE)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no such column"));
}

#[test]
fn missing_input_file_errors() {
    cmd()
        .args(["-i", "/no/such/file/definitely-missing.csv", "head"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("could not open input file"));
}

#[test]
fn empty_input_is_handled_gracefully() {
    // Completely empty stdin: no header, no rows. head should succeed with
    // empty output, not panic.
    cmd().arg("head").write_stdin("").assert().success();
}

#[test]
fn empty_input_stats_succeeds() {
    // stats on empty input should still produce the stats header and exit 0.
    cmd().arg("stats").write_stdin("").assert().success();
}

#[test]
fn from_json_invalid_json_errors() {
    cmd()
        .arg("from-json")
        .write_stdin("{not valid json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("valid JSON"));
}

#[test]
fn from_json_non_array_errors() {
    // A JSON object (not an array) should be rejected clearly.
    cmd()
        .arg("from-json")
        .write_stdin(r#"{"a":1}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("array"));
}

#[test]
fn ragged_rows_do_not_crash() {
    // Rows with too few / too many fields should be tolerated.
    let data = "a,b,c\n1,2\n4,5,6,7\n";
    cmd().arg("head").write_stdin(data).assert().success();
}

#[test]
fn frequency_via_stdin() {
    // city column: NYC appears twice, LA and SF once each.
    cmd()
        .args(["frequency", "city"])
        .write_stdin(SAMPLE)
        .assert()
        .success()
        // Exact full-stdout match: NYC x2 first, then LA/SF (count 1) by value asc.
        .stdout("value,count\nNYC,2\nLA,1\nSF,1\n");
}

#[test]
fn frequency_unknown_column_errors() {
    cmd()
        .args(["frequency", "nope"])
        .write_stdin(SAMPLE)
        .assert()
        .failure()
        .stderr(predicate::str::contains("no such column"));
}
