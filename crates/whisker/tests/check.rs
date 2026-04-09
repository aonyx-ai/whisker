use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

fn whisker() -> Command {
    Command::cargo_bin("whisker").expect("whisker binary should exist")
}

#[test]
fn check_fixture_directory_succeeds() {
    whisker()
        .args(["check", "tests/fixtures"])
        .assert()
        .success();
}

#[test]
fn check_nonexistent_path_fails() {
    whisker()
        .args(["check", "does/not/exist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn check_single_file_succeeds() {
    whisker()
        .args(["check", "tests/fixtures/wildcard_match.rs"])
        .assert()
        .success();
}

#[test]
fn check_with_keep_going_succeeds() {
    whisker()
        .args(["check", "--keep-going", "tests/fixtures"])
        .assert()
        .success();
}
