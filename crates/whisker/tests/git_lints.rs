use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

#[path = "support/fixture_repository.rs"]
mod fixture_repository;

use fixture_repository::{FixtureRepository, Standalone, git, write_lint_package, write_lockfile};

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

    let config = package.join(".config");
    std::fs::create_dir_all(&config).expect("the config directory should be created");
    std::fs::write(
        config.join("whisker.toml"),
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
///
/// The same reasoning removes a token and a release API. Neither belongs
/// near a fixture remote.
fn whisker(cache: &TempDir) -> Command {
    let mut command = Command::cargo_bin("whisker").expect("whisker binary should exist");
    command.env("WHISKER_CACHE_DIR", cache.path());
    command.env("CARGO_TARGET_DIR", shared_build_directory());
    command.env("GIT_CONFIG_GLOBAL", "/dev/null");
    command.env("GIT_CONFIG_SYSTEM", "/dev/null");
    command.env_remove("GH_TOKEN");
    command.env_remove("GITHUB_TOKEN");
    command.env_remove("WHISKER_GITHUB_API_URL");

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

/// Pins that the fixture helper ignores the git environment around the tests
///
/// The suite runs from a pre-commit hook, and git exports `GIT_DIR` and
/// `GIT_INDEX_FILE` to a hook in a linked worktree. A fixture that inherited
/// them once ran `git init` and `git add --all` against the checkout being
/// committed. The test sets the process environment, which nextest confines
/// to this test's own process.
#[test]
fn fixture_repository_ignores_an_ambient_git_environment() {
    let victim = tempfile::tempdir().expect("temporary directory should be created");
    std::fs::write(victim.path().join("file.txt"), "hello\n").expect("the file should be written");
    git(victim.path(), &["init", "--quiet", "-b", "main"]);
    git(victim.path(), &["add", "--all"]);
    let git_dir = victim.path().join(".git");
    let index_before = std::fs::read(git_dir.join("index")).expect("the index should be readable");
    let config_before =
        std::fs::read(git_dir.join("config")).expect("the config should be readable");
    unsafe {
        std::env::set_var("GIT_DIR", &git_dir);
        std::env::set_var("GIT_INDEX_FILE", git_dir.join("index"));
    }

    let rules = repository_with_one_lint("fixture.no-todo");

    unsafe {
        std::env::remove_var("GIT_DIR");
        std::env::remove_var("GIT_INDEX_FILE");
    }
    let index_after = std::fs::read(git_dir.join("index")).expect("the index should be readable");
    let config_after =
        std::fs::read(git_dir.join("config")).expect("the config should be readable");
    assert_eq!(
        index_after, index_before,
        "the fixture wrote the victim's index"
    );
    assert_eq!(
        config_after, config_before,
        "the fixture wrote the victim's config"
    );
    assert_eq!(
        rules.rev().len(),
        40,
        "the fixture should hold a commit of its own"
    );
}

/// Pins that a git environment around whisker cannot reach its checkout
///
/// A linter is run from a git hook, and a hook exports `GIT_DIR` and
/// `GIT_INDEX_FILE` pointing at the repository being committed. Gitoxide
/// reads those, so a checkout that honored them would write its index into
/// somebody else's repository and fail.
#[test]
fn check_with_git_lint_source_ignores_an_ambient_git_environment() {
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = repository_with_one_lint("fixture.no-todo");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), &rules.url(), rules.rev());

    whisker(&cache)
        .env("GIT_DIR", target.path().join(".git"))
        .env("GIT_INDEX_FILE", target.path().join(".git").join("index"))
        .env("GIT_WORK_TREE", target.path())
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[fixture.no-todo]"));
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
