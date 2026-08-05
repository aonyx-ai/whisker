use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

#[cfg(unix)]
#[path = "support/unreadable.rs"]
mod unreadable;

#[cfg(unix)]
use unreadable::make_unreadable;

/// Source that trips none of the lints the CLI runs
const CLEAN_SOURCE: &str = "pub fn answer() -> u32 {\n    42\n}\n";

/// An ignore file the walker cannot parse
///
/// An unclosed alternate group fails the walk on every platform. The other
/// trigger, an unreadable directory, does not exist on Windows.
const UNPARSABLE_IGNORE_FILE: &str = "{a,b\n";

/// A manifest for a standalone package with no dependencies
const MANIFEST: &str = "[package]\nname = \"target\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// Source that trips `lint.bool-param` and nothing else
///
/// The rule is syntactic, so it fires without semantic decorations.
const WARNING_SOURCE: &str = "pub fn set(flag: bool) {\n    let _ = flag;\n}\n";

/// Rust source that is not valid UTF-8
///
/// The trailing `0xff` makes `read_to_string` fail on this file.
const NOT_UTF8: &[u8] = b"fn main() {}\n\xff\n";

/// Creates a minimal Cargo package whose `src/lib.rs` holds `source`
///
/// A real package is the only target rust-analyzer can load. A loose `.rs`
/// file would let a test pass without semantic analysis.
fn package(source: &str) -> TempDir {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    std::fs::write(directory.path().join("Cargo.toml"), MANIFEST)
        .expect("manifest should be written");
    std::fs::create_dir(directory.path().join("src")).expect("src should be created");
    std::fs::write(directory.path().join("src").join("lib.rs"), source)
        .expect("source should be written");

    directory
}

/// Creates a package that holds one readable source and one non-UTF-8 file
///
/// The non-UTF-8 file is a sibling, not the crate root, so the package
/// still loads and the read failure is the one under test.
fn package_with_unreadable_source() -> TempDir {
    let directory = package(CLEAN_SOURCE);

    std::fs::write(directory.path().join("src").join("broken.rs"), NOT_UTF8)
        .expect("unreadable source should be written");

    directory
}

fn whisker() -> Command {
    Command::cargo_bin("whisker").expect("whisker binary should exist")
}

#[test]
fn check_current_directory_succeeds() {
    whisker().arg("check").assert().success();
}

#[test]
fn check_directory_without_sources_fails() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    whisker()
        .arg("check")
        .arg(directory.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("analyzed no files"));
}

#[test]
fn check_non_rust_file_fails() {
    whisker()
        .args(["check", "Cargo.toml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no grammar for `.toml` files"));
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
fn check_package_directory_succeeds() {
    let package = package(CLEAN_SOURCE);

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn check_package_whose_sources_a_gitignore_excludes_fails() {
    let package = package(CLEAN_SOURCE);
    std::fs::write(package.path().join(".gitignore"), "src/\n")
        .expect("gitignore should be written");

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("analyzed no files"));
}

/// Pins the exclusion source that lives entirely outside the project
///
/// The test points `GIT_CONFIG_GLOBAL` at a temporary config, so it neither
/// reads nor disturbs the developer's own settings.
#[test]
fn check_package_whose_sources_the_global_gitignore_excludes_fails() {
    let package = package(CLEAN_SOURCE);
    let git = tempfile::tempdir().expect("temporary directory should be created");
    let excludes = git.path().join("ignore");
    std::fs::write(&excludes, "src/\n").expect("global gitignore should be written");
    let config = git.path().join("config");
    std::fs::write(
        &config,
        format!("[core]\n\texcludesfile = {}\n", excludes.display()),
    )
    .expect("git configuration should be written");

    whisker()
        .env("GIT_CONFIG_GLOBAL", &config)
        .arg("check")
        .arg(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("analyzed no files"));
}

/// Pins that `--keep-going` survives a walk error and still fails the run
///
/// The discovery unit tests pin what each policy does, not which one the
/// CLI chooses, so only this test covers that wiring.
#[test]
fn check_package_with_an_unparsable_ignore_file_and_keep_going_reports_it_and_fails() {
    let package = package(CLEAN_SOURCE);
    std::fs::write(
        package.path().join("src").join(".gitignore"),
        UNPARSABLE_IGNORE_FILE,
    )
    .expect("gitignore should be written");

    whisker()
        .args(["check", "--keep-going"])
        .arg(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "error: failed to read an ignore file",
        ))
        .stderr(predicate::str::contains("error parsing glob '{a,b'"))
        .stderr(predicate::str::contains("failed to discover source files").not());
}

/// Pins that a walk error ends the run when `--keep-going` is not given
///
/// Both policies exit non-zero, so this also asserts the discovery context,
/// which the `--keep-going` path never prints.
#[test]
fn check_package_with_an_unparsable_ignore_file_fails() {
    let package = package(CLEAN_SOURCE);
    std::fs::write(
        package.path().join("src").join(".gitignore"),
        UNPARSABLE_IGNORE_FILE,
    )
    .expect("gitignore should be written");

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to discover source files"))
        .stderr(predicate::str::contains("failed to read an ignore file"))
        .stderr(predicate::str::contains("error parsing glob '{a,b'"));
}

#[cfg(unix)]
#[test]
fn check_package_with_an_unreadable_directory_and_keep_going_reports_it_and_fails() {
    let package = package(CLEAN_SOURCE);
    let locked = package.path().join("src").join("locked");
    std::fs::create_dir(&locked).expect("directory should be created");
    std::fs::write(locked.join("inner.rs"), CLEAN_SOURCE).expect("source should be written");
    let _unreadable = make_unreadable(&locked);

    whisker()
        .args(["check", "--keep-going"])
        .arg(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "error: failed to read a directory entry",
        ))
        .stderr(predicate::str::contains("failed to discover source files").not());
}

#[cfg(unix)]
#[test]
fn check_package_with_an_unreadable_directory_fails() {
    let package = package(CLEAN_SOURCE);
    let locked = package.path().join("src").join("locked");
    std::fs::create_dir(&locked).expect("directory should be created");
    std::fs::write(locked.join("inner.rs"), CLEAN_SOURCE).expect("source should be written");
    let _unreadable = make_unreadable(&locked);

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to discover source files"))
        .stderr(predicate::str::contains("failed to read a directory entry"));
}

#[test]
fn check_single_file_succeeds() {
    let package = package(CLEAN_SOURCE);

    whisker()
        .args(["check", "src/lib.rs"])
        .current_dir(package.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn check_with_deny_warnings_fails_on_warnings() {
    let package = package(WARNING_SOURCE);

    whisker()
        .args(["check", "--deny-warnings"])
        .arg(package.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error[lint.bool-param]"));
}

/// The non-UTF-8 source produces a read failure and no diagnostic, so only
/// that failure can make the exit code non-zero. The recovery path alone
/// prints the lowercase `error:` line the assertions accept.
#[test]
fn check_with_keep_going_and_unreadable_file_fails() {
    let package = package_with_unreadable_source();

    whisker()
        .args(["check", "--keep-going"])
        .arg(package.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "stream did not contain valid UTF-8",
        ))
        .stderr(predicate::str::contains("error: "))
        .stderr(predicate::str::contains("Error:").not());
}

#[test]
fn check_with_keep_going_succeeds() {
    let package = package(CLEAN_SOURCE);

    whisker()
        .args(["check", "--keep-going"])
        .arg(package.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

/// Without `--keep-going`, the same package ends the run. The error names
/// the failing file; anyhow renders the cause as a separate line.
#[test]
fn check_with_unreadable_file_and_no_keep_going_fails() {
    let package = package_with_unreadable_source();

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("Error:"))
        .stderr(predicate::str::contains("broken.rs"))
        .stderr(predicate::str::contains(
            "stream did not contain valid UTF-8",
        ));
}

#[test]
fn check_without_deny_warnings_reports_warnings_and_succeeds() {
    let package = package(WARNING_SOURCE);

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[lint.bool-param]"));
}
