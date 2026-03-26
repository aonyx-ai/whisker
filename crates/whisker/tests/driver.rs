use std::path::PathBuf;
use std::process::Command;

fn whisker_bin() -> PathBuf {
    env!("CARGO_BIN_EXE_whisker").into()
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn rustc_path() -> PathBuf {
    let rustup_home = std::env::var("RUSTUP_HOME").expect("RUSTUP_HOME not set");
    let toolchain = std::env::var("RUSTUP_TOOLCHAIN").expect("RUSTUP_TOOLCHAIN not set");
    PathBuf::from(rustup_home)
        .join("toolchains")
        .join(&toolchain)
        .join("bin/rustc")
}

fn run_driver(fixture_name: &str) -> std::process::Output {
    Command::new(whisker_bin())
        .env("__WHISKER_DRIVER", "1")
        .arg(rustc_path())
        .arg(fixture(fixture_name))
        .arg("--edition=2024")
        .arg("--crate-type=lib")
        .arg("--emit=metadata")
        .arg("-o")
        .arg("/dev/null")
        .output()
        .expect("failed to run whisker driver")
}

#[test]
fn driver_fires_whisker_lints() {
    let output = run_driver("wildcard_match.rs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("wildcard match arm"),
        "expected wildcard_match_arm lint in stderr, got:\n{stderr}"
    );
}

#[test]
fn driver_preserves_builtin_lints() {
    let output = run_driver("wildcard_match_with_dead_code.rs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("wildcard match arm"),
        "expected wildcard_match_arm lint in stderr, got:\n{stderr}"
    );
    assert!(
        stderr.contains("unused_function"),
        "expected dead_code warning for unused_function in stderr, got:\n{stderr}"
    );
}

#[test]
fn mode_detection_without_env_var() {
    let output = Command::new(whisker_bin())
        .arg("--some-arg")
        .output()
        .expect("failed to run whisker");

    assert!(
        !output.status.success(),
        "expected non-zero exit without __WHISKER_DRIVER"
    );
}
