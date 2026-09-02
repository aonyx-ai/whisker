use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

#[path = "support/fake_github.rs"]
mod fake_github;
#[path = "support/fixture_repository.rs"]
mod fixture_repository;
#[path = "support/mismatched_plugin.rs"]
mod mismatched_plugin;

use fake_github::{Answer, FakeGitHub};
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
///
/// This removes what the person running the tests holds. Their token
/// would otherwise reach a fixture server, and whisker would ask their
/// API about a repository these tests invented.
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

/// The remote every download test pins
///
/// The fake API answers for this host, so whisker asks about the remote.
/// The host also serves no git. A test that falls back to the source
/// therefore fails at once, and resolves no name.
const REMOTE: &str = "https://127.0.0.1/aonyx/rules";

/// Where the API serves the releases of that remote
const RELEASES: &str = "/repos/aonyx/rules/releases";

/// A commit of the right shape for the remote above
const REV: &str = "0123456789abcdef0123456789abcdef01234567";

/// Returns a whisker that asks `server` instead of GitHub
fn whisker_asking(cache: &TempDir, api: &str) -> Command {
    let mut command = whisker(cache);
    command.env("WHISKER_GITHUB_API_URL", api);

    command
}

/// Builds a lint package called `package` and returns its library
///
/// Every test names its package after itself, for the reason
/// [`repository_with_one_lint`] gives.
fn fixture_library(package: &str) -> PathBuf {
    let source = tempfile::tempdir().expect("temporary directory should be created");
    write_lint_package(source.path(), package, RULE, Standalone::Yes);

    build_cdylib(source.path(), package)
}

/// Returns a gzipped tar holding `libraries` at its root
fn archive_of(libraries: &[PathBuf]) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);

    for library in libraries {
        let name = library
            .file_name()
            .expect("a library should have a name")
            .to_string_lossy()
            .into_owned();
        builder
            .append_path_with_name(library, name)
            .expect("the library should be added");
    }

    builder
        .into_inner()
        .expect("the archive should be finished")
        .finish()
        .expect("the archive should be flushed")
}

/// Returns the SHA-256 of `bytes` in the spelling a sidecar uses
fn digest_of(bytes: &[u8]) -> String {
    use sha2::Digest as _;

    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);

    hex::encode(hasher.finalize())
}

/// Returns a release listing offering `assets` from `server`
fn listing(server: &FakeGitHub, assets: &[&str]) -> String {
    let assets: Vec<String> = assets
        .iter()
        .map(|name| {
            format!(
                r#"{{"name":"{name}","url":"{}/assets/{name}"}}"#,
                server.url()
            )
        })
        .collect();

    format!(r#"[{{"draft":false,"assets":[{}]}}]"#, assets.join(","))
}

/// Publishes an archive of `libraries` for `REV`, and returns its name
fn publish(server: &FakeGitHub, libraries: &[PathBuf]) -> String {
    let name = format!("{REV}-{}.tar.gz", abi_tag());
    let sidecar = format!("{name}.sha256");
    let bytes = archive_of(libraries);
    let digest = digest_of(&bytes);

    server.route(RELEASES, Answer::json(listing(server, &[&name, &sidecar])));
    server.route(&format!("/assets/{name}"), Answer::bytes(bytes));
    server.route(
        &format!("/assets/{sidecar}"),
        Answer::text(format!("{digest}  {name}\n")),
    );

    name
}

/// Pins the whole path from the question to the diagnostic
///
/// Whisker asks, downloads, checks the digest, unpacks, loads, and
/// reports.
#[test]
fn check_with_a_published_archive_loads_it_without_a_source() {
    let server = FakeGitHub::start();
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), REMOTE, REV);
    publish(&server, &[fixture_library("published_lint")]);

    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains(format!("warning[{RULE}]")))
        .stderr(predicate::str::contains("finish this before it ships"));

    assert!(
        cache.path().join("prebuilt").is_dir(),
        "the archive should have been kept"
    );
}

/// Pins that what whisker kept is read without asking anyone
#[test]
fn check_with_prebuilt_lints_in_the_cache_asks_nothing() {
    let server = FakeGitHub::start();
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), REMOTE, REV);
    publish(&server, &[fixture_library("cached_lint")]);

    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .success();
    let asked = server.seen().len();

    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains(format!("warning[{RULE}]")));

    assert_eq!(
        server.seen().len(),
        asked,
        "the second run should have asked nothing"
    );
}

/// Pins that a repository nobody built for stays quiet
///
/// Whisker says nothing and compiles the source, as it did before any of
/// this existed. A warning here would appear on every check of every
/// project whose lints are not published.
#[test]
fn check_without_a_matching_asset_says_nothing() {
    let server = FakeGitHub::start();
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), REMOTE, REV);
    server.route(
        RELEASES,
        Answer::json(listing(&server, &["other.tar.gz", "other.tar.gz.sha256"])),
    );

    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("warning: whisker cannot use").not());
}

/// Pins that a repository whisker cannot list is the same quiet case
#[test]
fn check_with_a_repository_the_api_does_not_know_says_nothing() {
    let server = FakeGitHub::start();
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), REMOTE, REV);

    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("warning: whisker cannot use").not());
}

/// An archive that does not match its digest is not unpacked, and is said
#[test]
fn check_with_a_wrong_digest_warns() {
    let server = FakeGitHub::start();
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), REMOTE, REV);
    let name = publish(&server, &[fixture_library("digest_lint")]);
    server.route(
        &format!("/assets/{name}.sha256"),
        Answer::text(format!("{}  {name}\n", "0".repeat(64))),
    );

    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("warning: whisker cannot use"))
        .stderr(predicate::str::contains("digest"));

    assert!(
        !cache.path().join("prebuilt").join("rules").exists(),
        "nothing should have been kept"
    );
}

/// An API that answers with a fault is worth saying, unlike a missing one
#[test]
fn check_with_a_failing_release_api_warns() {
    let server = FakeGitHub::start();
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), REMOTE, REV);
    server.route(RELEASES, Answer::failure(500));

    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("warning: whisker cannot use"))
        .stderr(predicate::str::contains("500"));
}

/// A private repository answers only a request that carries a token
#[test]
fn check_with_a_token_sends_it_to_the_api() {
    let server = FakeGitHub::start();
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), REMOTE, REV);
    publish(&server, &[fixture_library("token_lint")]);

    whisker_asking(&cache, server.url())
        .env("GH_TOKEN", "a-secret")
        .arg("check")
        .arg(target.path())
        .assert()
        .success();

    let carried = server
        .seen()
        .iter()
        .filter(|seen| seen.header("authorization") == Some("Bearer a-secret"))
        .count();

    assert_eq!(carried, server.seen().len(), "{:?}", server.seen());
}

/// A remote the API does not answer for is never asked about
///
/// A project that keeps its lints in a repository somewhere else must not
/// have that repository's name sent anywhere.
#[test]
fn check_with_a_remote_the_api_does_not_serve_asks_nothing() {
    let server = FakeGitHub::start();
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = repository_with_one_lint("unserved_lint", RULE);
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), &rules.url(), rules.rev());

    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains(format!("warning[{RULE}]")));

    assert!(server.seen().is_empty(), "{:?}", server.seen());
}

/// Clones the fixture at `rules` into `destination`, detached at its commit
///
/// # Panics
///
/// Panics if git is unavailable or any step fails.
fn clone_into(rules: &FixtureRepository, destination: &Path) {
    let run = |arguments: &[&str], directory: &Path| {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(directory)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap_or_else(|error| panic!("git {arguments:?} should run: {error}"));

        assert!(
            output.status.success(),
            "git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };

    let parent = destination.parent().expect("the path should have a parent");
    std::fs::create_dir_all(parent).expect("the parent should be created");

    let name = destination
        .file_name()
        .expect("the path should have a name")
        .to_string_lossy()
        .into_owned();

    run(&["clone", "--quiet", &rules.url(), &name], parent);
    run(
        &["checkout", "--quiet", "--detach", rules.rev()],
        destination,
    );
}

/// Pins that whisker compiles a checkout it already holds, and asks nobody
///
/// Whisker once asked a release API before it looked at what it held. A
/// project whose lints publish nothing then spent a request on every
/// check, and stopped working away from a network.
#[test]
fn check_with_a_checkout_already_present_asks_nothing() {
    let server = FakeGitHub::start();
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    let rules = repository_with_one_lint("warm_lint", RULE);
    let target = package(TODO_SOURCE);
    configure_git_lint(target.path(), REMOTE, rules.rev());

    // The first run cannot reach the remote, and fails. It leaves behind
    // the directory whisker keeps that remote's checkouts in, and the
    // run that matters needs a checkout there.
    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .failure();
    let remote = fetched_remote(&cache);
    clone_into(
        &rules,
        &cache.path().join("git").join(&remote).join(rules.rev()),
    );
    let asked = server.seen().len();

    whisker_asking(&cache, server.url())
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains(format!("warning[{RULE}]")));

    assert_eq!(
        server.seen().len(),
        asked,
        "the run should have asked nothing: {:?}",
        server.seen()
    );
}
