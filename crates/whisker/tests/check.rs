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
/// An unclosed alternate group fails the walk on every platform. The other
/// trigger, an unreadable directory, does not exist on Windows.
const UNPARSABLE_IGNORE_FILE: &str = "{a,b\n";

/// A manifest for a standalone package with no dependencies
const MANIFEST: &str = "[package]\nname = \"target\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// Source that trips `custom.no-todo` and nothing else
///
/// The rule is syntactic, so it fires without semantic decorations.
const WARNING_SOURCE: &str = "pub fn later() {\n    todo!()\n}\n";

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

/// Configures a package to run the example plugin's rule
///
/// Whisker links no rules, so a fixture that expects a diagnostic has to
/// name one. Pointing at a plugin in this repository rather than at a
/// purpose-built stub means the test exercises the whole path a real
/// project takes: whisker builds the package, loads the library, and
/// completes the handshake before a single file is walked.
fn with_no_todo(directory: TempDir) -> TempDir {
    write_lint_config(directory.path(), &example_lint());

    directory
}

/// Returns the example plugin this repository ships
fn example_lint() -> PathBuf {
    let lint = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/custom_lint");

    std::fs::canonicalize(lint).expect("the example plugin should be in this repository")
}

/// Returns the probe plugin whisker-rust's provider tests share
///
/// The probes read decorations, so they are what a fixture needs when the
/// diagnostic under test has to survive real semantic analysis.
fn decoration_probes() -> PathBuf {
    let lint = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../whisker-rust/tests/fixtures/lints/decoration_probes");

    std::fs::canonicalize(lint).expect("the probe plugin should be in this repository")
}

/// Writes a configuration naming one lint directory by absolute path
///
/// Every fixture needs one, whether or not it expects a diagnostic. The
/// search for a configuration climbs until it meets a `.git` directory, so
/// a fixture without a file of its own reads this repository's, and then
/// what the test actually runs is whatever rules whisker happens to be
/// configured with today.
///
/// The path goes through a TOML value rather than into quotes of our own,
/// so a checkout under a directory holding a quote still writes a
/// configuration whisker can read.
fn write_lint_config(directory: &Path, lint: &Path) {
    let lint = lint.to_str().expect("the lint path should be UTF-8");
    let lint = toml::Value::from(lint);

    std::fs::write(
        directory.join(".whisker.toml"),
        format!("[[lints]]\npath = {lint}\n"),
    )
    .expect("the configuration should be written");
}

/// Creates a package whose `src` holds Rust files no module declares
///
/// An orphan is a file the walk finds but no crate reaches, which is the
/// shape the coverage errors describe. The orphans live in a temporary
/// package because a checked-in orphan would make `whisker check .` fail
/// for everyone.
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

/// Creates a package whose `src` holds two sources that are not UTF-8
///
/// The crate root beside them is valid UTF-8 and trips no lints, so only
/// the two siblings fail. Two failing files let the error count separate
/// the two `--keep-going` modes. Discovery sorts, so a stopped run always
/// names `broken_first.rs`.
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
/// The fixture is a real Cargo package with a real crate graph, so whisker
/// can decorate the files under its `src` directory.
fn sample_project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../whisker-rust/tests/fixtures/sample_project")
}

/// Writes a Cargo project holding one analyzable file and one unreadable one
///
/// The fixture carries its own manifest because whisker loads the workspace
/// around the path it is given. `CARGO_TARGET_TMPDIR` is a scratch
/// directory, and nothing guarantees that a Cargo project sits above it.
///
/// `src/lib.rs` trips `fixture.wildcard-match-arm` and produces a
/// diagnostic. Whisker cannot read `src/not_utf8.rs`. Discovery sorts, so
/// `src/lib.rs` comes first, and a run without `--keep-going` collects the
/// diagnostic before `src/not_utf8.rs` stops the walk.
///
/// Each caller passes its own `name`, so two tests that run at once never
/// write the same directory.
fn mixed_project(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(root.join("src")).expect("should create the fixture directories");
    write_lint_config(&root, &decoration_probes());
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

/// Counts the failing files whisker reported on stderr
///
/// The count separates the two `--keep-going` modes: an aborted run reports
/// one failing file, a run that continues reports every one.
fn error_count(stderr: &str) -> usize {
    stderr.matches("error: ").count()
}

/// Counts how many times whisker printed the remedy for an unreachable file
///
/// Whisker prints the remedy once per run, not once per file; the count
/// pins that limit.
fn unreachable_help_count(stderr: &str) -> usize {
    stderr
        .matches("help: reference the file from a source the toolchain already loads")
        .count()
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

/// Pins that without `--keep-going` an uncoverable file ends the run, and
/// that the remedy still prints
///
/// The run collects the remedy during the walk; only the code after the
/// walk prints it.
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
            "nothing the toolchain loaded reaches it",
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

#[test]
fn check_package_whose_sources_the_configuration_excludes_fails() {
    let package = package(CLEAN_SOURCE);
    std::fs::write(
        package.path().join(".whisker.toml"),
        "ignore = [\"src/\"]\n",
    )
    .expect("configuration should be written");

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

/// Pins that a package the provider can fully reach produces lint output,
/// not coverage errors
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
        .stderr(predicate::str::contains(
            "warning[fixture.wildcard-match-arm]",
        ));
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
    let package = with_no_todo(package(WARNING_SOURCE));

    whisker()
        .args(["check", "--deny-warnings"])
        .arg(package.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error[custom.no-todo]"));
}

/// Pins the exit code to the failure recorded while walking the files, not
/// just to the diagnostics that came out of the walk
///
/// The crate root is clean, so only the recorded failures can make the exit
/// code non-zero. An error count of two shows the walk reported both
/// unreadable files.
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
/// Two errors mean the walk continued past the first orphan. The run still
/// prints the remedy once.
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

/// Pins that without `--keep-going` the walk stops at the first unreadable
/// file
///
/// The run reports only one of the two unreadable siblings, and the error
/// names the file so the failure stays attributable.
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

#[test]
fn check_without_deny_warnings_reports_warnings_and_succeeds() {
    let package = with_no_todo(package(WARNING_SOURCE));

    whisker()
        .arg("check")
        .arg(package.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[custom.no-todo]"));
}

/// Pins that a stopped run keeps the diagnostics from earlier files
///
/// A run without `--keep-going` stops at the first unreadable file. The
/// diagnostics collected before that point still reach the user.
#[test]
fn check_without_keep_going_keeps_diagnostics_from_earlier_files() {
    let root = mixed_project("check_without_keep_going");

    whisker()
        .current_dir(&root)
        .args(["check", "src"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error: src/not_utf8.rs"))
        .stderr(predicate::str::contains(
            "warning[fixture.wildcard-match-arm]",
        ));
}
