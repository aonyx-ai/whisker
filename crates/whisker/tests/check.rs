use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

fn whisker() -> Command {
    Command::cargo_bin("whisker").expect("whisker binary should exist")
}

#[test]
fn check_clean_fixture_directory_succeeds() {
    whisker()
        .args(["check", "tests/fixtures/clean"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn check_current_directory_succeeds() {
    whisker().arg("check").assert().success();
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
        .args(["check", "tests/fixtures/clean/wildcard_match.rs"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

// r[verify cli.check.deny-warnings]
#[test]
fn check_with_deny_warnings_fails_on_warnings() {
    whisker()
        .args(["check", "--deny-warnings", "tests/fixtures/warnings"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error[lint.bool-param]"));
}

#[test]
fn check_with_keep_going_succeeds() {
    whisker()
        .args(["check", "--keep-going", "tests/fixtures/clean"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

// r[verify cli.check.deny-warnings]
#[test]
fn check_without_deny_warnings_reports_warnings_and_succeeds() {
    whisker()
        .args(["check", "tests/fixtures/warnings"])
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[lint.bool-param]"));
}
