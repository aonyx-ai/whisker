use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

#[path = "support/fixture_repository.rs"]
mod fixture_repository;

use fixture_repository::{FixtureRepository, Standalone, write_lint_package, write_lockfile};

/// Source that trips the fixture lints and nothing whisker ships
const TODO_SOURCE: &str = "pub fn later() {\n    todo!()\n}\n";

/// A manifest for a standalone package with no dependencies
const MANIFEST: &str = "[package]\nname = \"target\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// A commit hash of the right shape that no fixture repository holds
const ABSENT_REV: &str = "0123456789abcdef0123456789abcdef01234567";

/// Creates a minimal Cargo package whose `src/lib.rs` holds `source`
fn package(source: &str) -> TempDir {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    std::fs::write(directory.path().join("Cargo.toml"), MANIFEST)
        .expect("manifest should be written");
    std::fs::create_dir(directory.path().join("src")).expect("src should be created");
    std::fs::write(directory.path().join("src").join("lib.rs"), source)
        .expect("source should be written");

    directory
}

/// Points the package's whisker configuration at one git lint source
///
/// Both values go through TOML rather than into quotes of our own, so a
/// checkout under a directory holding a quote still writes a configuration
/// whisker can read.
fn configure_git_lint(package: &Path, url: &str, rev: &str) {
    let url = toml::Value::from(url);
    let rev = toml::Value::from(rev);

    std::fs::write(
        package.join(".whisker.toml"),
        format!("[[lints]]\ngit = {url}\nrev = {rev}\n"),
    )
    .expect("configuration should be written");
}

/// Returns a whisker that keeps its fetched sources inside `cache`
///
/// Every test points the cache somewhere of its own, so a run neither
/// reads nor writes the cache of the person running it, and one test
/// cannot serve another's checkout.
///
/// The git configuration is emptied too. A fetch that quietly depended on
/// the identity in someone's `~/.gitconfig` passed everywhere it was
/// written and failed on every build agent, which is where whisker most
/// needs to work, so these tests run with the same nothing an agent has.
fn whisker(cache: &TempDir) -> Command {
    let mut command = Command::cargo_bin("whisker").expect("whisker binary should exist");
    command.env("WHISKER_CACHE_DIR", cache.path());
    command.env("CARGO_TARGET_DIR", shared_build_directory());
    command.env("GIT_CONFIG_GLOBAL", "/dev/null");
    command.env("GIT_CONFIG_SYSTEM", "/dev/null");

    command
}

/// Returns the build directory every fixture plugin in this file shares
///
/// Each test fetches into a cache of its own, so a fixture would otherwise
/// compile whisker-rust and everything under it from nothing, every test and
/// every run. The dependencies are the same ones each time, so one directory
/// outside the caches turns several cold builds into one. It sits under
/// `CARGO_TARGET_TMPDIR`, which cargo keeps beside the workspace's own build
/// output, so the warmth survives between runs as well as within one.
///
/// Whisker passes its environment to the cargo it runs, so this is also how
/// someone with `CARGO_TARGET_DIR` already set builds their plugins, and the
/// artifact search reads the paths cargo reports rather than assuming any.
fn shared_build_directory() -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("git_lint_plugins");
    std::fs::create_dir_all(&directory).expect("the build directory should be created");

    directory
}

/// Creates a repository holding one lint package
fn repository_with_one_lint(rule: &str) -> FixtureRepository {
    FixtureRepository::new(|directory| {
        write_lint_package(directory, "fixture_lint", rule, Standalone::Yes);
        write_lockfile(directory);
    })
}

/// Pins the whole path: fetch, build, handshake, lint, report
#[test]
fn check_with_git_lint_source_reports_its_diagnostic() {
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = repository_with_one_lint("fixture.no-todo");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), &rules.url(), rules.rev());

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[fixture.no-todo]"))
        .stderr(predicate::str::contains("finish this before it ships"));
}

/// A repository of rules is a workspace, and every member is a rule
///
/// This is the shape the extraction of whisker's own rules produces, so
/// one entry has to load all of them rather than the first one cargo
/// happened to finish.
#[test]
fn check_with_git_lint_workspace_loads_every_member() {
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = FixtureRepository::new(|directory| {
        std::fs::write(
            directory.join("Cargo.toml"),
            "[workspace]\nmembers = [\"lints/*\"]\nresolver = \"3\"\n",
        )
        .expect("the workspace manifest should be written");
        write_lint_package(
            &directory.join("lints").join("first"),
            "first",
            "fixture.first",
            Standalone::No,
        );
        write_lint_package(
            &directory.join("lints").join("second"),
            "second",
            "fixture.second",
            Standalone::No,
        );
        write_lockfile(directory);
    });
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), &rules.url(), rules.rev());

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[fixture.first]"))
        .stderr(predicate::str::contains("warning[fixture.second]"));
}

/// Names the remote and the commit when the remote cannot serve the pin
#[test]
fn check_with_git_lint_source_at_an_unknown_rev_fails() {
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = repository_with_one_lint("fixture.no-todo");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), &rules.url(), ABSENT_REV);

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(ABSENT_REV))
        .stderr(predicate::str::contains(rules.url()));
}

/// A checkout in the cache is used without asking the remote again
///
/// The remote is deleted between the two runs, so a second run that still
/// succeeds cannot have reached it. This is what makes a pinned source
/// cheap enough to sit in front of every check, and what keeps a check
/// working with no network at all.
#[test]
fn check_with_git_lint_source_reuses_the_checkout_without_the_remote() {
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = repository_with_one_lint("fixture.no-todo");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), &rules.url(), rules.rev());

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[fixture.no-todo]"));
    rules.remove();

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[fixture.no-todo]"));
}
