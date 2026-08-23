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

/// Points the package's whisker configuration at one pinned repository
fn configure_git_lint(package: &Path, url: &str, rev: &str) {
    let url = toml::Value::from(url);

    std::fs::write(
        package.join(".whisker.toml"),
        format!("[[lints]]\ngit = {url}\nrev = \"{rev}\"\n"),
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

/// Returns the build directory the plugin workspace uses
///
/// That workspace compiles against whisker-rust, and a target directory
/// inside the temporary checkout would compile the dependency again on
/// every run. One directory under the crate's own scratch space keeps a
/// repeat run cheap and stays out of anyone else's target directory.
fn shared_build_directory() -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("plugin-builds");
    std::fs::create_dir_all(&directory).expect("the shared build directory should be created");

    directory
}

/// Writes a workspace of two plugin packages and returns its root
///
/// The members differ in the node they answer to, so a source holding
/// both proves that each library was loaded rather than one of them
/// twice. Their dependencies are path dependencies on this repository,
/// which is what makes the handshake pass: one toolchain builds whisker
/// and both plugins.
fn workspace_of_lints() -> TempDir {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let crates = std::fs::canonicalize(crates).expect("the crates directory should exist");
    let rust = toml::Value::from(
        crates
            .join("whisker-rust")
            .to_str()
            .expect("the path should be UTF-8"),
    );
    let types = toml::Value::from(
        crates
            .join("whisker-types")
            .to_str()
            .expect("the path should be UTF-8"),
    );

    std::fs::write(
        directory.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"first\", \"second\"]\nresolver = \"3\"\n",
    )
    .expect("the workspace manifest should be written");

    for (name, rule, method) in [
        ("first", "workspace.first", "check_function_item"),
        ("second", "workspace.second", "check_macro_invocation"),
    ] {
        let member = directory.path().join(name);
        std::fs::create_dir(&member).expect("the member should be created");
        std::fs::create_dir(member.join("src")).expect("src should be created");
        std::fs::write(
            member.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
                 publish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n\
                 whisker-rust = {{ path = {rust}, default-features = false }}\n\
                 whisker-types = {{ path = {types} }}\n"
            ),
        )
        .expect("the member manifest should be written");
        std::fs::write(
            member.join("src").join("lib.rs"),
            format!(
                "use whisker_rust::RustLintPass;\n\
                 use whisker_types::{{DecoratedNode, Diagnostic, RuleId, Severity}};\n\n\
                 pub struct Flag;\n\n\
                 impl RustLintPass for Flag {{\n\
                 \x20   fn {method}(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {{\n\
                 \x20       vec![Diagnostic::new(\n\
                 \x20           RuleId::new(\"{rule}\"),\n\
                 \x20           Severity::Warn,\n\
                 \x20           \"the member fired\".into(),\n\
                 \x20           node.span(),\n\
                 \x20       )]\n\
                 \x20   }}\n\
                 }}\n\n\
                 whisker_rust::export_lints![Flag];\n"
            ),
        )
        .expect("the member source should be written");
    }

    directory
}

/// Pins where a pinned repository is looked for, and what is said if it
/// is not there
///
/// The cache directory is the whole contract of this step: a run resolves
/// a repository and a commit to one path and reads it, and the error has
/// to name that path, because a reader who wants to prime the cache or
/// clear it has nothing else to go on.
#[test]
fn check_with_a_git_lint_that_is_not_cached_names_the_checkout() {
    let target = package(TODO_SOURCE);
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    configure_git_lint(
        target.path(),
        "https://example.com/rules",
        "0123456789abcdef0123456789abcdef01234567",
    );

    whisker()
        .env("WHISKER_CACHE_DIR", cache.path())
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("holds no checkout"))
        .stderr(predicate::str::contains("https://example.com/rules"))
        .stderr(predicate::str::contains(
            "0123456789abcdef0123456789abcdef01234567",
        ));
}

/// Pins that a configured directory may be a workspace of plugins
///
/// One entry naming a repository of rules is the reason this matters: a
/// shared rule set is a workspace, and loading only its first library
/// would run a fraction of the rules while looking like it ran them all.
#[test]
fn check_with_a_workspace_of_lints_reports_every_member() {
    let target = package(TODO_SOURCE);
    let workspace = workspace_of_lints();
    configure_lint(target.path(), workspace.path());

    whisker()
        .env("CARGO_TARGET_DIR", shared_build_directory())
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[workspace.first]"))
        .stderr(predicate::str::contains("warning[workspace.second]"));
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
