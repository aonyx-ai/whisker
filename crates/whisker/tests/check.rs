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
/// A glob with an unclosed alternate group is the cheapest way to make the
/// walk fail on any platform, which matters because the other way of failing
/// it - a directory nobody may read - does not exist on Windows.
const UNPARSABLE_IGNORE_FILE: &str = "{a,b\n";

/// A manifest for a standalone package with no dependencies
const MANIFEST: &str = "[package]\nname = \"target\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// Source that trips `lint.bool-param` and nothing else
///
/// The rule is purely syntactic, so this fires whether or not the decoration
/// provider resolved anything, which keeps the severity tests independent of
/// how much semantic information was available.
const WARNING_SOURCE: &str = "pub fn set(flag: bool) {\n    let _ = flag;\n}\n";

/// Bytes that are a Rust source file everywhere except that they are not UTF-8
///
/// The trailing `0xff` is not a valid UTF-8 sequence, so `read_to_string`
/// fails on this file while the package around it still loads.
const NOT_UTF8: &[u8] = b"fn main() {}\n\xff\n";

/// Creates a minimal Cargo package whose `src/lib.rs` holds `source`
///
/// The tests that expect a clean run need a target rust-analyzer can actually
/// load, and a real package is the only thing that qualifies. Loose `.rs`
/// files belonging to no crate would let a test assert success over source no
/// decoration provider ever understood, and pointing the tests at whisker's
/// own source instead would tie them to whisker staying lint-clean, which is a
/// different claim than the one they are making.
fn package(source: &str) -> TempDir {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    std::fs::write(directory.path().join("Cargo.toml"), MANIFEST)
        .expect("manifest should be written");
    std::fs::create_dir(directory.path().join("src")).expect("src should be created");
    std::fs::write(directory.path().join("src").join("lib.rs"), source)
        .expect("source should be written");

    directory
}

/// Creates a package holding one readable source and one that is not UTF-8
///
/// The unreadable file is a sibling rather than the crate root so the package
/// still loads: the failure under test is whisker reading the file, not Cargo
/// refusing the target.
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

// r[verify cli.discovery.empty]
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

// r[verify cli.discovery.explicit-target]
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

// r[verify cli.discovery.empty]
// r[verify cli.discovery.ignore-files]
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

// r[verify cli.config.ignore]
#[test]
fn check_package_whose_sources_the_configuration_excludes_fails() {
    let package = package(CLEAN_SOURCE);
    std::fs::write(
        package.path().join("Cargo.toml"),
        format!("{MANIFEST}\n[workspace]\n\n[workspace.metadata.whisker]\nignore = [\"src/\"]\n"),
    )
    .expect("manifest should be written");

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("analyzed no files"));
}

/// Pins the exclusion source that lives entirely outside the project
///
/// The user's global gitignore is the last of the four sources the spec
/// promises and the only one whose location whisker never sees: the walker has
/// to read git's own configuration to find it. Nothing inside a checkout can
/// reveal that the lookup stopped happening, so a run would simply start
/// linting whatever that file was hiding, and the first person to notice would
/// be whoever configured it. Pointing `GIT_CONFIG_GLOBAL` at a temporary
/// config keeps the test hermetic - git gives that variable precedence over
/// both `$HOME/.gitconfig` and the XDG config - so it neither reads nor
/// disturbs the developer's own settings.
// r[verify cli.discovery.empty]
// r[verify cli.discovery.ignore-files]
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
/// Both halves matter and neither is visible from the discovery unit tests,
/// which pin what a policy does once it has been chosen and say nothing about
/// which one the CLI chooses. A run that reported the failure and exited zero
/// would be a linter telling a CI job that source it never read is fine, and a
/// run that reported nothing would be worse.
// r[verify cli.discovery.walk-errors]
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
        .stderr(predicate::str::contains("failed to discover source files").not());
}

/// Pins that a walk error ends the run when `--keep-going` is not given
///
/// The absence of the reported-and-continued message is what distinguishes
/// this from the `--keep-going` case: both exit non-zero, so an exit code
/// alone would not notice the two policies being swapped.
// r[verify cli.discovery.walk-errors]
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
        .stderr(predicate::str::contains("failed to read an ignore file"));
}

// r[verify cli.discovery.walk-errors]
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

// r[verify cli.discovery.walk-errors]
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

// r[verify cli.check.deny-warnings]
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

/// Pins the exit code to the failure recorded while walking the files, not
/// just to the diagnostics that came out of the walk
///
/// The package holds one source that is not valid UTF-8, so reading it fails
/// and `--keep-going` records that failure and moves on. The readable sibling
/// is clean, so no diagnostic survives the run, which means the exit code can
/// only be non-zero if the recorded failure is folded into the final outcome.
///
/// The assertion distinguishes the recovering path from the aborting one:
/// both mention the underlying I/O error and both exit 1, but only recovery
/// prints a lowercase `error:` line and lets the run continue.
// r[verify cli.check.keep-going]
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

/// Pins the hard-failure path that the `--keep-going` test is distinguished
/// from
///
/// Without the flag the same package ends the run, and the error names the
/// file it came from so a failure deep in a walk stays attributable. The
/// cause arrives as a separate line because anyhow renders the context chain
/// as a report rather than a single line.
// r[verify cli.check.keep-going]
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

// r[verify cli.check.deny-warnings]
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
