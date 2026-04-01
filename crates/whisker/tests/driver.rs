use std::path::PathBuf;

use assert_cmd::Command;
use predicates::str::contains;

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

fn driver_cmd(fixture_name: &str) -> Command {
    let mut cmd = Command::cargo_bin("whisker").expect("whisker binary not found");
    cmd.env("__WHISKER_DRIVER", "1")
        .arg(rustc_path())
        .arg(fixture(fixture_name))
        .arg("--edition=2024")
        .arg("--crate-type=lib")
        .arg("--emit=metadata")
        .arg("-o")
        .arg("/dev/null");
    cmd
}

// r[verify driver.register-lints]
#[test]
fn driver_fires_whisker_lints() {
    driver_cmd("wildcard_match.rs")
        .assert()
        .stderr(contains("wildcard match arm"));
}

// r[verify driver.preserve-existing-lints]
#[test]
fn driver_preserves_builtin_lints() {
    driver_cmd("wildcard_match_with_dead_code.rs")
        .assert()
        .stderr(contains("wildcard match arm"))
        .stderr(contains("unused_function"));
}

// r[verify driver.mode-detection]
#[test]
fn mode_detection_without_env_var() {
    Command::cargo_bin("whisker")
        .expect("whisker binary not found")
        .arg("--some-arg")
        .assert()
        .failure();
}
