use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

/// Source that trips the example lint and none of the built-ins
const TODO_SOURCE: &str = "pub fn later() {\n    todo!()\n}\n";

/// A manifest for a standalone package with no dependencies
const MANIFEST: &str = "[package]\nname = \"target\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// Creates a minimal Cargo package whose `src/lib.rs` holds `source`
///
/// A real package is the only target rust-analyzer can load, exactly as in
/// the check tests.
fn package(source: &str) -> TempDir {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    std::fs::write(directory.path().join("Cargo.toml"), MANIFEST)
        .expect("manifest should be written");
    std::fs::create_dir(directory.path().join("src")).expect("src should be created");
    std::fs::write(directory.path().join("src").join("lib.rs"), source)
        .expect("source should be written");

    directory
}

/// Points the package's whisker configuration at one custom lint path
///
/// The path goes through a TOML value rather than into quotes of our own,
/// so a checkout under a directory holding a quote still writes a
/// configuration whisker can read.
fn configure_lint(package: &Path, lint_path: &Path) {
    let lint_path = lint_path.to_str().expect("the lint path should be UTF-8");
    let lint_path = toml::Value::from(lint_path);

    std::fs::write(
        package.join(".whisker.toml"),
        format!("[[lints]]\npath = {lint_path}\n"),
    )
    .expect("configuration should be written");
}

/// Returns the example plugin this repository ships
fn example_lint() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/custom_lint");
    std::fs::canonicalize(path).expect("the example lint should exist")
}

fn whisker() -> Command {
    Command::cargo_bin("whisker").expect("whisker binary should exist")
}

/// Pins the whole path: configure, compile, handshake, lint, report
///
/// One toolchain builds both sides here, as the setup documentation asks
/// of a real user, so the handshake passes for the same reason theirs
/// does.
#[test]
fn check_with_custom_lint_reports_its_diagnostic() {
    let target = package(TODO_SOURCE);
    configure_lint(target.path(), &example_lint());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[custom.no-todo]"))
        .stderr(predicate::str::contains("finish this before it ships"));
}

#[test]
fn check_with_lint_path_that_does_not_exist_fails() {
    let target = package(TODO_SOURCE);
    configure_lint(target.path(), Path::new("does/not/exist"));

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "failed to load the project's custom lints",
        ))
        .stderr(predicate::str::contains("does not exist"));
}

/// Pins the remedy for the most likely authoring mistake
///
/// The decoy lint package has no dependencies, so the test builds it in
/// about a second and never gets as far as loading it.
#[test]
fn check_with_lint_that_builds_no_cdylib_fails() {
    let target = package(TODO_SOURCE);
    let lint = package("pub fn not_a_lint() {}");
    configure_lint(target.path(), lint.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("crate-type = [\"cdylib\"]"));
}

/// Pins the error for a dynamic library that is not a whisker plugin
#[test]
fn check_with_lint_that_exports_no_declaration_fails() {
    let target = package(TODO_SOURCE);
    let lint = package("pub fn not_a_lint() {}");
    std::fs::write(
        lint.path().join("Cargo.toml"),
        format!("{MANIFEST}\n[lib]\ncrate-type = [\"cdylib\"]\n"),
    )
    .expect("manifest should be written");
    configure_lint(target.path(), lint.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a whisker lint plugin"))
        .stderr(predicate::str::contains("export_lints!"));
}
