// r[verify cli.check.coverage]
// r[verify cli.check.keep-going]
use std::path::{Path, PathBuf};
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

/// Creates a package whose `src` holds Rust files no module declares
///
/// An orphan is a file the walk finds but no crate reaches, which is exactly
/// the shape the coverage errors describe. Building them in a temporary
/// package rather than checking them in matters: an orphan committed to this
/// repository would make `whisker check .` fail for everyone, which is what
/// the earlier `tests/fixtures` directory did before it was removed.
fn package_with_orphans(names: &[&str]) -> TempDir {
    let directory = package(CLEAN_SOURCE);

    for name in names {
        std::fs::write(
            directory.path().join("src").join(format!("{name}.rs")),
            CLEAN_SOURCE,
        )
        .expect("orphan source should be written");
    }

    directory
}

/// Creates a package holding two sources that are not UTF-8, and one that is
///
/// The unreadable files are siblings rather than the crate root so the
/// package still loads: the failure under test is whisker reading a file, not
/// Cargo refusing the target. There are two of them because the count is the
/// only thing that separates the two `--keep-going` modes — both report a
/// failure and both exit non-zero, so a `contains` assertion cannot tell them
/// apart. Both sort before `lib.rs`, and discovery sorts, so a run that stops
/// at the first failure has reported exactly one.
fn package_with_unreadable_sources() -> TempDir {
    let directory = package(CLEAN_SOURCE);

    for name in ["broken_first", "broken_second"] {
        std::fs::write(
            directory.path().join("src").join(format!("{name}.rs")),
            NOT_UTF8,
        )
        .expect("unreadable source should be written");
    }

    directory
}

fn whisker() -> Command {
    Command::cargo_bin("whisker").expect("whisker binary should exist")
}

/// Returns the fixture workspace that whisker-rust's own tests analyze
///
/// It is a real Cargo package with a real crate graph, so files under its
/// `src` directory are the only ones in this repository that whisker can
/// actually decorate.
fn sample_project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../whisker-rust/tests/fixtures/sample_project")
}

/// Writes a Cargo project holding one analyzable file and one unreadable one
///
/// `CARGO_TARGET_TMPDIR` is a scratch directory, not a Cargo project, and
/// whisker loads the workspace around the path it is given before it reads
/// a single file. A fixture written straight into the scratch directory
/// therefore only works when that directory happens to sit under a project,
/// which is true for a default `CARGO_TARGET_DIR` and false for anyone who
/// moved theirs. Writing the manifest states the precondition instead of
/// inheriting it.
///
/// `src/lib.rs` trips a rule that needs type information, so a run that
/// reaches it produces a diagnostic; `src/not_utf8.rs` cannot be read at
/// all. They are named so the analyzable one is walked first, because
/// `discover_files` sorts and a run without `--keep-going` stops at the
/// first failure.
///
/// Each caller passes its own `name` so that two tests running at once
/// never write the same directory.
fn mixed_project(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(root.join("src")).expect("should create the fixture directories");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"mixed_project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("should write the fixture manifest");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub enum Color {\n    Red,\n    Green,\n}\n\npub fn match_on_enum(color: Color) {\n    match color {\n        Color::Red => {}\n        _ => {}\n    }\n}\n",
    )
    .expect("should write the fixture crate root");
    std::fs::write(root.join("src/not_utf8.rs"), [0xff, 0xfe, 0x00])
        .expect("should write the fixture file that is not UTF-8");

    root
}

/// Counts how many times whisker reported a file it could not get through
///
/// The count is what separates the two `--keep-going` modes: one file
/// reported means the run stopped at the first failure, every file reported
/// means it carried on. A `contains` assertion cannot tell them apart.
fn error_count(stderr: &str) -> usize {
    stderr.matches("error: ").count()
}

/// Counts how many times whisker printed the remedy for an unreachable file
///
/// The remedy is deliberately printed once per run rather than once per
/// file, so that a directory of orphans does not bury the diagnostics.
fn unreachable_help_count(stderr: &str) -> usize {
    stderr
        .matches("help: add the file to a crate's module tree")
        .count()
}

/// Pins that this crate itself holds no Rust source belonging to no crate
///
/// An earlier version of this test asserted the opposite, because checked-in
/// orphan fixtures made a clean run impossible. Those fixtures are gone, and
/// asserting success is the stronger claim: it fails if anyone reintroduces
/// source the decoration provider cannot reach.
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

/// Pins that without `--keep-going` an uncoverable file ends the run, and
/// that the remedy is printed before it does
///
/// The remedy still has to reach the user: a run that says a file cannot be
/// analyzed without saying what to do about it leaves them nowhere to go, and
/// the remedy is collected during the walk, so nothing would print it if the
/// run simply stopped.
#[test]
fn check_orphan_directory_reports_no_coverage() {
    let package = package_with_orphans(&["stray"]);

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no decoration provider covers this file",
        ))
        .stderr(predicate::str::contains("stray.rs"))
        .stderr(predicate::function(|stderr: &str| error_count(stderr) == 1))
        .stderr(predicate::function(|stderr: &str| {
            unreachable_help_count(stderr) == 1
        }));
}

#[test]
fn check_orphan_file_reports_no_coverage() {
    let package = package_with_orphans(&["stray"]);

    whisker()
        .args(["check", "src/stray.rs"])
        .current_dir(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no decoration provider covers this file",
        ))
        .stderr(predicate::str::contains(
            "no crate in that workspace reaches it",
        ));
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

/// Pins that a package the provider can fully reach produces lint output
/// rather than coverage errors
///
/// Without this the coverage tests could all pass against a provider that
/// covered nothing at all, since every assertion they make is about failure.
#[test]
fn check_real_crate_produces_diagnostics_not_coverage_errors() {
    whisker()
        .current_dir(sample_project())
        .args(["check", "src"])
        .assert()
        .success()
        .stderr(predicate::str::contains("no decoration provider covers this file").not())
        .stderr(predicate::str::contains("warning[lint.wildcard-match-arm]"));
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
/// The count is what distinguishes recovery from aborting: carrying on means
/// every unreadable file is reported, stopping means only the first is.
// r[verify cli.check.keep-going]
#[test]
fn check_with_keep_going_and_unreadable_file_fails() {
    let package = package_with_unreadable_sources();

    whisker()
        .args(["check", "--keep-going"])
        .arg(package.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "stream did not contain valid UTF-8",
        ))
        .stderr(predicate::str::contains("broken_first.rs"))
        .stderr(predicate::str::contains("broken_second.rs"))
        .stderr(predicate::function(|stderr: &str| error_count(stderr) == 2));
}

/// Pins that `--keep-going` reports every uncoverable file, not just the first
///
/// The counts are what separate the two modes: one error means the run
/// stopped at the first orphan, two means it carried on. A `contains`
/// assertion cannot tell them apart. The remedy is still printed once,
/// because a directory of orphans would otherwise bury the diagnostics.
// r[verify cli.check.coverage]
// r[verify cli.check.keep-going]
#[test]
fn check_with_keep_going_reports_every_orphan_file() {
    let package = package_with_orphans(&["first_stray", "second_stray"]);

    whisker()
        .args(["check", "--keep-going"])
        .arg(package.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("first_stray.rs"))
        .stderr(predicate::str::contains("second_stray.rs"))
        .stderr(predicate::function(|stderr: &str| error_count(stderr) == 2))
        .stderr(predicate::function(|stderr: &str| {
            unreachable_help_count(stderr) == 1
        }));
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

/// Pins the stopping path that the `--keep-going` test is distinguished from
///
/// Without the flag the walk ends at the first file it cannot read, so only
/// one of the two unreadable siblings is ever reported. The error names the
/// file it came from so a failure deep in a walk stays attributable.
// r[verify cli.check.keep-going]
#[test]
fn check_with_unreadable_file_and_no_keep_going_fails() {
    let package = package_with_unreadable_sources();

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("broken_first.rs"))
        .stderr(predicate::str::contains(
            "stream did not contain valid UTF-8",
        ))
        .stderr(predicate::function(|stderr: &str| error_count(stderr) == 1));
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

/// Stopping at a bad file must not throw away the good files before it
///
/// A run without `--keep-going` stops at the first file it cannot get
/// through, which used to mean returning an error and discarding every
/// diagnostic collected so far. The work was already done and the findings
/// were already true, so the user lost them for no reason.
#[test]
fn check_without_keep_going_keeps_diagnostics_from_earlier_files() {
    let root = mixed_project("check_without_keep_going");

    whisker()
        .current_dir(&root)
        .args(["check", "src"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error: src/not_utf8.rs"))
        .stderr(predicate::str::contains("warning[lint.wildcard-match-arm]"));
}
