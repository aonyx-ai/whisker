use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

fn whisker() -> Command {
    Command::cargo_bin("whisker").expect("whisker binary should exist")
}

/// The fixtures under `tests/fixtures/clean` must follow every convention
/// whisker enforces. A semantic rule finds nothing while the provider cannot
/// resolve its subject, so a hidden violation breaks this test later, when
/// the provider improves. Fixtures that must be flagged belong in
/// `tests/fixtures/warnings`.
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
        .args(["check", "tests/fixtures/clean/exhaustive_match.rs"])
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

/// The fixture is not valid UTF-8, so the read fails and produces no
/// diagnostic. Only the recorded failure can make the exit code non-zero.
/// The assertion names the `error: {path}:` line because only the recovering
/// path prints it; aborting prints `Error: check {path}:`. The `.rs.bin`
/// name keeps directory walks away from the fixture, so the test names it
/// directly.
#[test]
fn check_with_keep_going_and_unreadable_file_fails() {
    whisker()
        .args([
            "check",
            "--keep-going",
            "tests/fixtures/invalid_utf8.rs.bin",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "error: tests/fixtures/invalid_utf8.rs.bin: stream did not contain valid UTF-8",
        ))
        .stderr(predicate::str::contains("Error:").not());
}

#[test]
fn check_with_keep_going_succeeds() {
    whisker()
        .args(["check", "--keep-going", "tests/fixtures/clean"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

/// Pins the hard-failure path that the `--keep-going` test has to be
/// distinguished from
///
/// Without the flag the same fixture ends the run, and the error names the
/// file it came from so a failure deep in a walk stays attributable. The
/// cause arrives as a separate line because anyhow renders the context chain
/// as a report rather than a single line.
// r[verify cli.check.keep-going]
#[test]
fn check_with_unreadable_file_and_no_keep_going_fails() {
    whisker()
        .args(["check", "tests/fixtures/invalid_utf8.rs.bin"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "Error: check tests/fixtures/invalid_utf8.rs.bin",
        ))
        .stderr(predicate::str::contains(
            "stream did not contain valid UTF-8",
        ));
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
