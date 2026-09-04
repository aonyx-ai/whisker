use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

#[path = "support/protocol_two_plugin.rs"]
mod protocol_two_plugin;

use protocol_two_plugin::write_protocol_two_lint_package;

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

    write_config(package, &format!("[[lints]]\npath = {lint_path}\n"));
}

/// Writes `contents` to the configuration file of the project at `package`
fn write_config(package: &Path, contents: &str) {
    let config = package.join(".config");
    std::fs::create_dir_all(&config).expect("the config directory should be created");
    std::fs::write(config.join("whisker.toml"), contents).expect("configuration should be written");
}

/// Points the package's whisker configuration at one pinned repository
fn configure_git_lint(package: &Path, url: &str, rev: &str) {
    let url = toml::Value::from(url);

    write_config(
        package,
        &format!("[[lints]]\ngit = {url}\nrev = \"{rev}\"\n"),
    );
}

/// Returns the example plugin this repository ships
fn example_lint() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/custom_lint");
    std::fs::canonicalize(path).expect("the example lint should exist")
}

/// Returns a whisker with nothing of the caller's environment in it
///
/// A token or a release API of the person who runs the tests must not
/// reach a fixture remote. No run here reads either one.
fn whisker() -> Command {
    let mut command = Command::cargo_bin("whisker").expect("whisker binary should exist");
    command.env_remove("GH_TOKEN");
    command.env_remove("GITHUB_TOKEN");
    command.env_remove("WHISKER_GITHUB_API_URL");

    command
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
                 impl whisker_rust::DeclaresRules for Flag {{\n\
                 \x20   fn rules(&self) -> Vec<RuleId> {{\n\
                 \x20       vec![RuleId::new(\"{rule}\")]\n\
                 \x20   }}\n\
                 }}\n\n\
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

/// Pins that a project runs only the rules it names
#[test]
fn check_with_an_enabled_rule_runs_only_that_rule() {
    let target = package(TODO_SOURCE);
    let lints = workspace_of_lints();
    let lint_path = toml::Value::from(lints.path().to_str().expect("the path should be UTF-8"));
    write_config(
        target.path(),
        &format!("[rules]\nenable = [\"workspace.second\"]\n\n[[lints]]\npath = {lint_path}\n"),
    );

    whisker()
        .env("CARGO_TARGET_DIR", shared_build_directory())
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[workspace.second]"))
        .stderr(predicate::str::contains("workspace.first").not());
}

/// Pins that a disabled rule reports nothing and the others still do
#[test]
fn check_with_a_disabled_rule_runs_the_rest() {
    let target = package(TODO_SOURCE);
    let lints = workspace_of_lints();
    let lint_path = toml::Value::from(lints.path().to_str().expect("the path should be UTF-8"));
    write_config(
        target.path(),
        &format!("[rules]\ndisable = [\"workspace.first\"]\n\n[[lints]]\npath = {lint_path}\n"),
    );

    whisker()
        .env("CARGO_TARGET_DIR", shared_build_directory())
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[workspace.second]"))
        .stderr(predicate::str::contains("workspace.first").not());
}

/// Pins that a name no lint reports is refused
///
/// A misspelled rule would otherwise disable nothing and say nothing,
/// which reads exactly like a rule that found no fault. The error names
/// what the loaded lints do report, so the fix is in front of the reader.
#[test]
fn check_with_a_rule_no_lint_reports_fails() {
    let target = package(TODO_SOURCE);
    let lints = workspace_of_lints();
    let lint_path = toml::Value::from(lints.path().to_str().expect("the path should be UTF-8"));
    write_config(
        target.path(),
        &format!("[rules]\ndisable = [\"workspace.frist\"]\n\n[[lints]]\npath = {lint_path}\n"),
    );

    whisker()
        .env("CARGO_TARGET_DIR", shared_build_directory())
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("workspace.frist"))
        .stderr(predicate::str::contains("workspace.first"));
}

/// Pins that naming both lists is refused rather than resolved
#[test]
fn check_with_both_rule_lists_fails() {
    let target = package(TODO_SOURCE);
    write_config(
        target.path(),
        "[rules]\nenable = [\"a.b\"]\ndisable = [\"c.d\"]\n",
    );

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("both"));
}

/// Pins that a plugin from an older protocol still loads
///
/// A protocol is raised when the declaration gains a field, and such a
/// plugin ends before it. Whisker knows the older layout, so it reads
/// what the plugin has and treats the rest as absent, and the rules of an
/// older plugin still run.
///
/// This is why an optional capability belongs in the declaration and not
/// on a trait: a `#[repr(C)]` struct has offsets whisker can reason
/// about, and a vtable has none it can measure.
#[test]
fn check_with_a_plugin_from_an_older_protocol_loads_it() {
    let target = package(TODO_SOURCE);
    let lints = tempfile::tempdir().expect("temporary directory should be created");
    write_protocol_two_lint_package(lints.path(), "older_lint", "older.fired");
    configure_lint(target.path(), lints.path());

    whisker()
        .env("CARGO_TARGET_DIR", shared_build_directory())
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[older.fired]"));
}

/// Pins that an older plugin's rules cannot be named
///
/// It declares none, so whisker has nothing to check a name against and
/// says so, rather than accepting a name it cannot honour.
#[test]
fn check_naming_a_rule_of_an_older_plugin_fails() {
    let target = package(TODO_SOURCE);
    let lints = tempfile::tempdir().expect("temporary directory should be created");
    write_protocol_two_lint_package(lints.path(), "older_named", "older.fired");
    let lint_path = toml::Value::from(lints.path().to_str().expect("the path should be UTF-8"));
    write_config(
        target.path(),
        &format!("[rules]\ndisable = [\"older.fired\"]\n\n[[lints]]\npath = {lint_path}\n"),
    );

    whisker()
        .env("CARGO_TARGET_DIR", shared_build_directory())
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("older.fired"));
}

/// Pins the error for a repository that cannot be reached
///
/// The remote here does not resolve, so the fetch fails before any
/// protocol runs. The test asserts that the report names both the remote
/// and the commit, because neither alone tells a reader which entry of
/// their configuration to look at.
#[test]
fn check_with_a_git_lint_that_cannot_be_fetched_names_the_source() {
    let target = package(TODO_SOURCE);
    let cache = tempfile::tempdir().expect("temporary directory should be created");
    configure_git_lint(
        target.path(),
        "https://whisker.invalid/rules",
        "0123456789abcdef0123456789abcdef01234567",
    );

    whisker()
        .env("WHISKER_CACHE_DIR", cache.path())
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to check out"))
        .stderr(predicate::str::contains("https://whisker.invalid/rules"))
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
