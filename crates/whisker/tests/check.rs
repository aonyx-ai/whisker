use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

fn whisker() -> Command {
    Command::cargo_bin("whisker").expect("whisker binary should exist")
}

/// Pins that a directory of conforming sources produces no output at all
///
/// Everything under `tests/fixtures/clean` has to conform to the conventions
/// whisker enforces, not merely fail to trip a lint. A lint that needs
/// semantic decorations returns nothing when it cannot resolve its subject,
/// so a fixture that violates such a rule would sit here silently until the
/// decoration provider improved and then break this test for the right
/// reason. Fixtures belong in `tests/fixtures/warnings` if they are meant to
/// be flagged.
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

/// Pins the exit code to the failure recorded while walking the files, not
/// just to the diagnostics that came out of the walk
///
/// The fixture is a Rust source file that is not valid UTF-8, so reading it
/// fails and `--keep-going` records that failure and moves on. No diagnostic
/// survives the run, which means the exit code can only be non-zero if the
/// recorded failure is folded into the final outcome.
///
/// The fixture is named `.rs.bin` rather than `.rs` so that no directory walk
/// ever picks it up. A deliberately unreadable source file anywhere under this
/// crate would otherwise turn every `whisker check` over the crate into a hard
/// error, so the test names the file directly instead.
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
            "stream did not contain valid UTF-8",
        ));
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
