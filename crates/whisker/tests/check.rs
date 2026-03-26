use std::path::PathBuf;
use std::process::Command;

fn whisker_bin() -> PathBuf {
    env!("CARGO_BIN_EXE_whisker").into()
}

fn sample_project_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample_project/Cargo.toml")
}

#[test]
fn check_runs_lints_on_sample_project() {
    let output = Command::new(whisker_bin())
        .args(["check", "--manifest-path"])
        .arg(sample_project_manifest())
        .output()
        .expect("failed to run whisker check");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("wildcard match arm"),
        "expected wildcard_match_arm lint in stderr, got:\n{stderr}"
    );
}

#[test]
fn check_forwards_extra_args() {
    let output = Command::new(whisker_bin())
        .args(["check", "--manifest-path"])
        .arg(sample_project_manifest())
        .args(["--", "-p", "sample_project"])
        .output()
        .expect("failed to run whisker check");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("wildcard match arm"),
        "expected wildcard_match_arm lint with -p forwarded, got:\n{stderr}"
    );
}
