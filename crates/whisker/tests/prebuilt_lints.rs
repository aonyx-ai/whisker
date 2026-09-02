use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

#[path = "support/fixture_repository.rs"]
mod fixture_repository;
#[path = "support/mismatched_plugin.rs"]
mod mismatched_plugin;

use fixture_repository::{FixtureRepository, Standalone, write_lint_package, write_lockfile};
use mismatched_plugin::write_mismatched_lint_package;

/// Source that trips the fixture lint and nothing whisker ships
const TODO_SOURCE: &str = "pub fn later() {\n    todo!()\n}\n";

/// A manifest for a standalone package with no dependencies
const MANIFEST: &str = "[package]\nname = \"target\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// The rule the fixture lint reports under
const RULE: &str = "fixture.no-todo";

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

/// Returns a whisker that keeps everything it fetches inside `cache`
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
/// This is the directory `git_lints.rs` uses. Both files compile the same
/// fixture against the same whisker, so one warm directory serves both.
fn shared_build_directory() -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("git_lint_plugins");
    std::fs::create_dir_all(&directory).expect("the build directory should be created");

    directory
}

/// Returns the tag this whisker asks publishers of prebuilt lints for
///
/// The test reads it from the binary rather than deriving it, so that the
/// name a publisher would use and the name whisker looks for are the same
/// string by construction.
fn abi_tag() -> String {
    let output = Command::cargo_bin("whisker")
        .expect("whisker binary should exist")
        .arg("abi")
        .output()
        .expect("whisker abi should run");

    assert!(output.status.success(), "whisker abi failed");

    String::from_utf8(output.stdout)
        .expect("whisker should write UTF-8")
        .trim()
        .to_owned()
}

/// Returns the file name a cdylib named `stem` builds to on this platform
fn library_name(stem: &str) -> String {
    format!(
        "{}{stem}.{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_EXTENSION
    )
}

/// Creates a repository holding one lint package called `package`
///
/// Every test names its package after itself. The fixtures of this file
/// and of `git_lints.rs` compile into one build directory, so two tests
/// that built a package of the same name would replace each other's
/// library while a third read it.
fn repository_with_one_lint(package: &str, rule: &str) -> FixtureRepository {
    let package = package.to_owned();

    FixtureRepository::new(move |directory| {
        write_lint_package(directory, &package, rule, Standalone::Yes);
        write_lockfile(directory);
    })
}

/// Returns the name whisker gave the remote it fetched into `cache`
///
/// The seed below puts libraries where whisker looks for them, and that
/// place comes from the remote. This reads the name back from whisker's
/// own cache. A change to how whisker spells a remote therefore fails
/// the test.
fn fetched_remote(cache: &TempDir) -> PathBuf {
    let git = cache.path().join("git");
    let mut remotes: Vec<PathBuf> = std::fs::read_dir(&git)
        .expect("whisker should have fetched into the cache")
        .map(|entry| entry.expect("the cache should be readable").path())
        .collect();
    remotes.sort();

    assert_eq!(remotes.len(), 1, "expected one remote: {remotes:?}");

    remotes
        .pop()
        .expect("the cache should hold one remote")
        .file_name()
        .expect("the remote should have a name")
        .into()
}

/// Puts `libraries` where whisker looks for prebuilt lints of `rev`
fn seed_prebuilt(cache: &TempDir, remote: &Path, rev: &str, libraries: &[PathBuf]) -> PathBuf {
    let destination = cache
        .path()
        .join("prebuilt")
        .join(remote)
        .join(rev)
        .join(abi_tag());

    std::fs::create_dir_all(&destination).expect("the prebuilt directory should be created");

    for library in libraries {
        let name = library.file_name().expect("a library should have a name");
        std::fs::copy(library, destination.join(name)).expect("the library should be copied");
    }

    destination
}

/// Removes every way to reach the lints except the prebuilt directory
///
/// After this there is no checkout to load, no remote to fetch it from,
/// and no source to compile. A run that still reports the lint can only
/// have read what was seeded.
fn strand(cache: &TempDir, rules: FixtureRepository) {
    std::fs::remove_dir_all(cache.path().join("git")).expect("the checkouts should be removable");
    rules.remove();
}

/// Pins that whisker loads prebuilt lints and needs nothing else
///
/// A source build produces the libraries a publisher would ship, so the
/// first run makes them. The second run has nothing left to fetch or
/// compile. The diagnostic can therefore come only from the seeded
/// libraries.
#[test]
fn check_with_prebuilt_lints_loads_them_without_a_source() {
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = repository_with_one_lint("loaded_lint", RULE);
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), &rules.url(), rules.rev());

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success();

    let remote = fetched_remote(&cache);
    let built = shared_build_directory()
        .join("release")
        .join(library_name("loaded_lint"));
    seed_prebuilt(&cache, &remote, rules.rev(), &[built]);
    strand(&cache, rules);

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains(format!("warning[{RULE}]")))
        .stderr(predicate::str::contains("finish this before it ships"));
}

/// Pins that every library in a prebuilt directory is loaded
///
/// A repository of rules is a workspace, and every member builds its own
/// library. One configured entry therefore ships as many libraries as
/// the repository has rules. Whisker must load all of them.
#[test]
fn check_with_several_prebuilt_libraries_loads_every_one() {
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = FixtureRepository::new(|directory| {
        std::fs::write(
            directory.join("Cargo.toml"),
            "[workspace]\nmembers = [\"lints/*\"]\nresolver = \"3\"\n",
        )
        .expect("the workspace manifest should be written");
        write_lint_package(
            &directory.join("lints").join("first"),
            "workspace_first",
            "fixture.first",
            Standalone::No,
        );
        write_lint_package(
            &directory.join("lints").join("second"),
            "workspace_second",
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
        .success();

    let remote = fetched_remote(&cache);
    let release = shared_build_directory().join("release");
    let built = vec![
        release.join(library_name("workspace_first")),
        release.join(library_name("workspace_second")),
    ];
    seed_prebuilt(&cache, &remote, rules.rev(), &built);
    strand(&cache, rules);

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[fixture.first]"))
        .stderr(predicate::str::contains("warning[fixture.second]"));
}

/// Pins that a prebuilt directory holding nothing loadable is not an answer
///
/// An unpack that was interrupted leaves an empty directory behind.
/// Whisker has to treat that as an absent one and fall back, rather than
/// fail a run over a cache entry it can replace.
#[test]
fn check_with_an_empty_prebuilt_directory_builds_from_source() {
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = repository_with_one_lint("empty_lint", RULE);
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), &rules.url(), rules.rev());

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success();

    let remote = fetched_remote(&cache);
    seed_prebuilt(&cache, &remote, rules.rev(), &[]);

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains(format!("warning[{RULE}]")));
}

/// Pins that the handshake still runs on a library whisker did not build
///
/// A library that fails the handshake carries a tag that misdescribes
/// it. A compile of the source would hide that from everyone who trusts
/// the tag. The run fails instead, and names the directory to delete.
#[test]
fn check_with_a_prebuilt_that_fails_the_handshake_fails() {
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = repository_with_one_lint("handshake_lint", RULE);
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), &rules.url(), rules.rev());

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .success();

    let remote = fetched_remote(&cache);
    let mismatched = tempfile::tempdir().expect("temporary directory should be created");
    write_mismatched_lint_package(mismatched.path(), "mismatched_lint");
    let built = build_cdylib(mismatched.path(), "mismatched_lint");
    let destination = seed_prebuilt(&cache, &remote, rules.rev(), &[built]);

    whisker(&cache)
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("protocol version"))
        .stderr(predicate::str::contains(
            destination.to_string_lossy().into_owned(),
        ));
}

/// Builds the package at `directory` and returns the library it produced
///
/// # Panics
///
/// Panics if cargo cannot be run or the build fails.
fn build_cdylib(directory: &Path, name: &str) -> PathBuf {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target = shared_build_directory();

    let output = Command::new(cargo)
        .args(["build", "--release"])
        .current_dir(directory)
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .expect("cargo should run");

    assert!(
        output.status.success(),
        "cargo build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let library = target.join("release").join(library_name(name));

    assert!(library.is_file(), "expected a library at {library:?}");

    library
}
