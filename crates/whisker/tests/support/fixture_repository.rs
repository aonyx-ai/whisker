use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A git repository built for one test, and the commit it holds
///
/// The fetch under test speaks the git wire protocol, so a fixture has to
/// be a real repository rather than a directory that looks like one. The
/// repository is served over `file://`, which runs the same `upload-pack`
/// conversation a remote would: a plain path would let git take a local
/// shortcut and the test would stop covering the protocol.
pub struct FixtureRepository {
    directory: TempDir,
    rev: String,
}

impl FixtureRepository {
    /// Creates a repository whose single commit holds what `build` wrote
    ///
    /// # Panics
    ///
    /// Panics if git is unavailable or any step fails, because a fixture
    /// that did not build cannot produce a meaningful test result.
    pub fn new(build: impl FnOnce(&Path)) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        build(directory.path());

        git(directory.path(), &["init", "--quiet", "-b", "main"]);
        git(directory.path(), &["add", "--all"]);
        git(
            directory.path(),
            &[
                "-c",
                "user.name=Whisker Test",
                "-c",
                "user.email=whisker@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--message",
                "Add lints",
            ],
        );

        let rev = git(directory.path(), &["rev-parse", "HEAD"]);
        let rev = rev.trim().to_owned();

        Self { directory, rev }
    }

    /// Returns the `file://` remote whisker should fetch from
    pub fn url(&self) -> String {
        format!("file://{}", self.directory.path().display())
    }

    /// Returns the commit a configuration should pin
    pub fn rev(&self) -> &str {
        &self.rev
    }

    /// Takes the repository off the filesystem, leaving the pin dangling
    ///
    /// A checkout that survives this is one whisker read from its cache
    /// rather than from the network.
    ///
    /// # Panics
    ///
    /// Panics if the directory cannot be removed.
    pub fn remove(self) {
        let Self { directory, .. } = self;

        directory
            .close()
            .expect("the fixture repository should be removable");
    }
}

/// Runs one git command in `directory` and returns its standard output
///
/// The fixture ignores the configuration of whoever runs the tests. A
/// global `commit.gpgsign`, a `core.hooksPath`, or a commit template would
/// otherwise decide whether the suite passes.
///
/// # Panics
///
/// Panics if git cannot be run or exits unsuccessfully.
fn git(directory: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap_or_else(|error| panic!("git {arguments:?} should run: {error}"));

    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("git should write UTF-8")
}

/// Returns the whisker crates a fixture plugin depends on
///
/// A plugin outside this repository writes git dependencies pinned to a
/// whisker revision. A fixture cannot: the whisker under test is this
/// working tree, including changes that are not committed anywhere. It
/// uses absolute path dependencies for the same reason a plugin uses a
/// pin, which is to be built against exactly the whisker that will load it.
///
/// # Panics
///
/// Panics if the whisker crates cannot be located.
pub fn whisker_crates() -> PathBuf {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

    std::fs::canonicalize(crates).expect("the whisker crates should be in this repository")
}

/// Writes a lint package that flags one macro, and returns its manifest
///
/// The package is a trimmed copy of the shipped example: the point of
/// these tests is where a plugin comes from, so the plugin itself stays as
/// small as a plugin can be while still exercising the whole path.
///
/// # Panics
///
/// Panics if the package cannot be written.
pub fn write_lint_package(directory: &Path, name: &str, rule: &str, standalone: Standalone) {
    let crates = whisker_crates();
    let crates = crates.to_str().expect("the path should be UTF-8");
    let workspace = match standalone {
        Standalone::Yes => "[workspace]\n\n",
        Standalone::No => "",
    };

    std::fs::create_dir_all(directory.join("src")).expect("the package should be created");
    std::fs::write(
        directory.join("Cargo.toml"),
        format!(
            "{workspace}[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \
             \"2024\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n\
             whisker-rust = {{ path = \"{crates}/whisker-rust\", default-features = false \
             }}\nwhisker-types = {{ path = \"{crates}/whisker-types\" }}\n"
        ),
    )
    .expect("the manifest should be written");
    std::fs::write(
        directory.join("src").join("lib.rs"),
        lint_source(name, rule),
    )
    .expect("the source should be written");
}

/// Whether a fixture package carries a workspace table of its own
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Standalone {
    Yes,
    No,
}

/// Returns the source of a lint that flags one macro by name
///
/// The rule fires on `todo!` and reports the rule id it is given, so a
/// test with two lints can tell which of them spoke.
fn lint_source(name: &str, rule: &str) -> String {
    let pass = name.to_uppercase();

    format!(
        r#"use whisker_rust::RustLintPass;
use whisker_types::{{DecoratedNode, Diagnostic, RuleId, Severity}};

/// Flags every `todo!` left in the code
pub struct {pass};

impl RustLintPass for {pass} {{
    fn check_macro_invocation(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {{
        let Some(macro_node) = node.child_by_field_name("macro") else {{
            return Vec::new();
        }};
        if macro_node.text() != "todo" {{
            return Vec::new();
        }}

        vec![Diagnostic::new(
            RuleId::new("{rule}"),
            Severity::Warn,
            "finish this before it ships".into(),
            node.span(),
        )]
    }}
}}

whisker_rust::export_lints![{pass}];
"#
    )
}

/// Resolves the fixture's dependencies so its build can run locked
///
/// Whisker builds a git source with `--locked`, because a pin that let
/// dependencies drift would not pin much. A repository of rules commits
/// its lockfile for that reason, and a fixture has to do the same.
///
/// # Panics
///
/// Panics if cargo cannot resolve the fixture.
pub fn write_lockfile(directory: &Path) {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());

    let output = Command::new(cargo)
        .args(["generate-lockfile", "--offline"])
        .current_dir(directory)
        .output()
        .expect("cargo should run");

    assert!(
        output.status.success(),
        "cargo generate-lockfile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
