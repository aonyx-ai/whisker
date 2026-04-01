use std::path::PathBuf;

use assert_cmd::Command;
use predicates::str::contains;

fn sample_project_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample_project/Cargo.toml")
}

// r[verify cli.check]
// r[verify cli.check.manifest-path]
#[test]
fn check_runs_lints_on_sample_project() {
    Command::cargo_bin("whisker")
        .expect("whisker binary not found")
        .args(["check", "--manifest-path"])
        .arg(sample_project_manifest())
        .assert()
        .stderr(contains("wildcard match arm"));
}

// r[verify cli.check.extra-args]
#[test]
fn check_forwards_extra_args() {
    Command::cargo_bin("whisker")
        .expect("whisker binary not found")
        .args(["check", "--manifest-path"])
        .arg(sample_project_manifest())
        .args(["--", "-p", "sample_project"])
        .assert()
        .stderr(contains("wildcard match arm"));
}
