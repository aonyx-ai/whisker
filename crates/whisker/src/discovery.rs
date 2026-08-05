use std::path::{Path, PathBuf};

use anyhow::Context as _;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{DirEntry, Match, WalkBuilder};
use whisker_types::Language;

use crate::config::WhiskerConfig;

mod walk_error_policy;
mod walk_failure;

pub use walk_error_policy::WalkErrorPolicy;
use walk_failure::WalkFailure;

/// The source files a `whisker check` run will inspect
///
/// Discovery is deliberately conservative about what it hands on. A linter
/// that reports on generated output is worse than useless, so the walk honors
/// the same exclusions the project's own tooling honors - `.gitignore`,
/// `.ignore`, `.git/info/exclude`, the user's global gitignore, and hidden
/// files - before applying whisker's own [`IgnorePattern`]s on top.
///
/// Those files are honored whether or not there is a repository around them,
/// which is where whisker departs from what `ripgrep` does by default. A
/// `.gitignore` inside an exported tarball, a vendored dependency, or a
/// checkout whose `.git` directory was stripped still describes which files
/// the project generates, and that description does not stop being true
/// because the repository is gone.
///
/// Failures are carried alongside the files rather than thrown away. An
/// unreadable directory quietly shrinks the scan and an unparsable ignore file
/// quietly reshapes it, either of which would let a run report success over
/// source it never opened, so the caller always gets the chance to act on
/// them.
///
/// [`IgnorePattern`]: crate::config::IgnorePattern
#[derive(Debug)]
pub struct Discovery {
    files: Vec<PathBuf>,
    errors: Vec<anyhow::Error>,
}

impl Discovery {
    /// Discovers the files to lint beneath `path`
    ///
    /// A `path` naming a single file is returned as-is, provided whisker has a
    /// grammar for it. Ignore rules describe what a broad sweep should skip,
    /// and a user who types a file's name has already made the decision the
    /// rules exist to automate; refusing at that point would look like whisker
    /// silently doing nothing. Whether whisker can parse the file at all is a
    /// different question, and not one the user can overrule: handing a
    /// `Cargo.toml` to the Rust grammar produces a tree nothing matches, so it
    /// would be reported as clean without ever having been understood.
    ///
    /// The same exemption applies to a directory named on the command line: it
    /// is walked even when a pattern would have excluded it, and the pattern
    /// that excluded it is not reapplied to the files inside, since asking for
    /// a directory is asking for what it contains. Patterns that match
    /// something further down still prune it.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` cannot be resolved, if `path` names a file
    /// written in a language whisker has no grammar for, if a configured
    /// ignore pattern is not valid gitignore syntax, or - under
    /// [`WalkErrorPolicy::Fail`] - if a directory entry or an ignore file
    /// cannot be read.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = WhiskerConfig::load(Path::new("."))?;
    /// let discovery = Discovery::run(Path::new("."), &config, WalkErrorPolicy::Fail)?;
    ///
    /// for file in discovery.files() {
    ///     println!("{}", file.display());
    /// }
    /// ```
    // r[impl cli.discovery.ignore-files]
    pub fn run(
        path: &Path,
        config: &WhiskerConfig,
        on_error: WalkErrorPolicy,
    ) -> anyhow::Result<Self> {
        let root = std::fs::canonicalize(path)
            .with_context(|| format!("failed to resolve {}", path.display()))?;

        // r[impl cli.discovery.explicit-target]
        if root.is_file() {
            let Some(extension) = root.extension() else {
                anyhow::bail!(
                    "whisker cannot check {}: it has no file extension, so there is nothing to \
                     tell whisker which language to parse it as",
                    path.display()
                );
            };

            let extension = extension.to_string_lossy();

            let Some(_language) = Language::from_extension(&extension) else {
                anyhow::bail!(
                    "whisker cannot check {}: there is no grammar for `.{extension}` files",
                    path.display()
                );
            };

            return Ok(Self {
                files: vec![path.to_path_buf()],
                errors: Vec::new(),
            });
        }

        let excludes = build_excludes(config)?;

        let walk = WalkBuilder::new(&root)
            .require_git(false)
            .filter_entry(move |entry| {
                let is_dir = entry
                    .file_type()
                    .is_some_and(|file_type| file_type.is_dir());

                match excludes.matched(entry.path(), is_dir) {
                    Match::None => true,
                    Match::Ignore(_) => false,
                    Match::Whitelist(_) => true,
                }
            })
            .build();

        let mut files = Vec::new();
        let mut errors = Vec::new();

        for entry in walk {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    let context = WalkFailure::classify(&error).context();
                    let error = anyhow::Error::new(error).context(context);

                    // r[impl cli.discovery.walk-errors]
                    match on_error {
                        WalkErrorPolicy::Fail => return Err(error),
                        WalkErrorPolicy::ReportAndContinue => {
                            errors.push(error);
                            continue;
                        }
                    }
                }
            };

            // r[impl cli.discovery.walk-errors]
            match attached_error(&entry) {
                None => {}
                Some(error) => match on_error {
                    WalkErrorPolicy::Fail => return Err(error),
                    WalkErrorPolicy::ReportAndContinue => errors.push(error),
                },
            }

            let Some(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }

            let Some(extension) = entry.path().extension() else {
                continue;
            };
            let Some(_language) = Language::from_extension(&extension.to_string_lossy()) else {
                continue;
            };

            files.push(rebase(path, &root, entry.path()));
        }

        files.sort();

        Ok(Self { files, errors })
    }

    /// Returns the discovered files, in a stable order
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert!(discovery.files().iter().all(|file| file.exists()));
    /// ```
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Returns the failures the walk survived
    ///
    /// Always empty under [`WalkErrorPolicy::Fail`], which returns the first
    /// failure instead of recording it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// for error in discovery.errors() {
    ///     eprintln!("error: {error:#}");
    /// }
    /// ```
    pub fn errors(&self) -> &[anyhow::Error] {
        &self.errors
    }
}

/// Returns the failure the walker recorded against `entry`, if there was one
///
/// An ignore file whose syntax the walker cannot parse does not abandon the
/// walk and does not arrive as an `Err`: the entry comes back intact with the
/// failure hanging off it, and the rules that file was supposed to express are
/// silently absent. That is the same class of harm as a directory that cannot
/// be read - the set of linted files changes and nothing says so - which is
/// why discovery treats the two the same way.
fn attached_error(entry: &DirEntry) -> Option<anyhow::Error> {
    let error = entry.error()?;

    Some(anyhow::Error::msg(error.to_string()).context("failed to read an ignore file"))
}

/// Compiles the configured ignore patterns into a matcher
///
/// The matcher is anchored at the configured root rather than at the directory
/// being scanned, which is why it is built from the configuration: `examples/`
/// has to mean the same thing whether the run starts at the workspace root or
/// inside a member crate three levels down. Within that root the patterns
/// behave exactly as they would in a `.gitignore` written there, so one
/// containing an interior slash names a path relative to the root while one
/// without it matches at any depth.
///
/// # Errors
///
/// Returns an error naming the offending pattern if it is not valid gitignore
/// syntax.
fn build_excludes(config: &WhiskerConfig) -> anyhow::Result<Gitignore> {
    let mut builder = GitignoreBuilder::new(config.root());

    for pattern in config.ignore() {
        builder
            .add_line(None, pattern.as_str())
            .with_context(|| format!("failed to read the ignore pattern `{pattern}`"))?;
    }

    builder
        .build()
        .context("failed to compile the configured ignore patterns")
}

/// Returns `entry` spelled the way the caller spelled `original`
///
/// The walk runs over a resolved root so that patterns anchored at the
/// workspace root line up with the paths being matched against them. What
/// reaches a diagnostic, though, should read the way the user asked for it:
/// someone who ran `whisker check .` wants `./src/main.rs`, not the machine's
/// idea of where that file really lives.
fn rebase(original: &Path, root: &Path, entry: &Path) -> PathBuf {
    original.join(entry.strip_prefix(root).unwrap_or(entry))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    use tempfile::TempDir;

    use super::*;
    use crate::config::IgnorePattern;

    /// A virtual workspace manifest with no members
    ///
    /// Members would have to be buildable to be useful, and the tests that
    /// reach for a manifest are about the metadata table rather than the crate
    /// graph.
    const EMPTY_WORKSPACE: &str = "[workspace]\nresolver = \"3\"\nmembers = []\n";

    /// A directory whose permissions are restored when this value is dropped
    ///
    /// The walk-error tests have to survive a panic between clearing a
    /// directory's mode and putting it back: a temporary directory left at
    /// mode `000` cannot be removed, so an assertion failure would be followed
    /// by a second, unrelated failure from [`TempDir`]'s own cleanup, and the
    /// leftover directory would outlive the test run.
    #[cfg(unix)]
    struct Unreadable<'a> {
        directory: &'a Path,
    }

    #[cfg(unix)]
    impl Drop for Unreadable<'_> {
        fn drop(&mut self) {
            make_readable(self.directory);
        }
    }

    /// Panics unless `directory` reads back in an order that needs sorting
    ///
    /// The walk hands entries on in whatever order the filesystem stores them,
    /// which on the filesystems whisker is developed against is a hash order
    /// that a short list of names can easily come out of already sorted. A
    /// test asserting a sorted result over such a list asserts nothing about
    /// the sort, so this makes the assumption it rests on explicit and fails
    /// where it stops holding instead of quietly passing.
    ///
    /// # Panics
    ///
    /// Panics if `directory` cannot be read, or if it reads back sorted.
    fn assert_stored_out_of_order(directory: &Path) {
        let names: Vec<String> = std::fs::read_dir(directory)
            .expect("directory should be read")
            .map(|entry| {
                entry
                    .expect("entry should be read")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        let mut sorted = names.clone();
        sorted.sort();

        assert_ne!(
            names,
            sorted,
            "{} reads back in sorted order, so a sorted result proves nothing about whisker \
             sorting anything; give the test more files or different names",
            directory.display()
        );
    }

    /// Builds a configuration anchored at `root` with the given patterns
    fn config(root: &Path, patterns: &[&str]) -> WhiskerConfig {
        let root = std::fs::canonicalize(root).expect("root should resolve");
        let patterns = patterns
            .iter()
            .map(|pattern| IgnorePattern::new(*pattern))
            .collect();

        WhiskerConfig::new(root, patterns)
    }

    /// Returns the discovered files as slash-separated paths relative to `root`
    fn discovered(discovery: &Discovery, root: &Path) -> Vec<String> {
        discovery
            .files()
            .iter()
            .map(|file| {
                file.strip_prefix(root)
                    .unwrap_or(file)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    /// Restores default permissions so the temporary directory can be removed
    #[cfg(unix)]
    fn make_readable(directory: &Path) {
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o755))
            .expect("permissions should be restored");
    }

    /// Makes `directory` impossible to read until the returned value is dropped
    ///
    /// A process with enough privilege - root in a CI container, most often -
    /// can still read a mode `000` directory. A caller that quietly bowed out
    /// there would report a passing test having asserted nothing at all about
    /// the walk-error path, which is the very failure mode this crate exists
    /// to prevent, so this stops loudly instead.
    ///
    /// # Panics
    ///
    /// Panics if `directory` cannot have its mode cleared, or if it is still
    /// readable afterwards.
    #[cfg(unix)]
    fn make_unreadable(directory: &Path) -> Unreadable<'_> {
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o000))
            .expect("permissions should be set");

        let unreadable = Unreadable { directory };

        assert!(
            std::fs::read_dir(directory).is_err(),
            "{} is still readable at mode 000, so the walk-error tests cannot exercise anything; \
             run them without privileges that bypass file permissions",
            directory.display()
        );

        unreadable
    }

    /// Creates a temporary directory holding the given relative files
    fn tree(files: &[&str]) -> TempDir {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        for file in files {
            let path = directory.path().join(file);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("parent directory should be created");
            }
            std::fs::write(&path, "fn main() {}").expect("file should be written");
        }

        directory
    }

    #[test]
    fn build_excludes_with_invalid_pattern_returns_error() {
        let directory = tree(&[]);
        let config = config(directory.path(), &["{a,b"]);

        let error = build_excludes(&config).expect_err("pattern should be rejected");

        assert!(
            format!("{error:#}").contains("failed to read the ignore pattern `{a,b`"),
            "error should name the offending pattern: {error:#}"
        );
    }

    #[test]
    fn rebase_with_entry_outside_root_returns_the_entry() {
        let entry = Path::new("/elsewhere/main.rs");

        let path = rebase(Path::new("."), Path::new("/project"), entry);

        assert_eq!(path, entry);
    }

    #[test]
    fn rebase_with_entry_under_root_rejoins_the_original() {
        let entry = Path::new("/project/src/main.rs");

        let path = rebase(Path::new("."), Path::new("/project"), entry);

        assert_eq!(path, Path::new("./src/main.rs"));
    }

    /// Pins the exclusions whisker's own manifest declares
    ///
    /// Whisker is meant to be run over whisker, and the fixture projects it
    /// keeps around are deliberately full of the violations its rules look
    /// for. They also sit outside the workspace's crate graph, so nothing can
    /// decorate them. A pattern that stopped matching would therefore not fail
    /// loudly; it would fill an otherwise clean run with diagnostics about
    /// files that exist to be wrong, which is how a project learns to ignore
    /// its own linter. The synthetic tests above prove the matching rules, and
    /// this one proves they were spelled correctly here.
    // r[verify cli.config.ignore]
    #[test]
    fn run_over_whiskers_own_workspace_excludes_its_fixture_projects() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let config = WhiskerConfig::load(&root).expect("whisker's own configuration should load");

        let discovery = Discovery::run(&root, &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        let files = discovered(&discovery, &root);
        assert!(
            files.contains(&"crates/whisker/src/discovery.rs".to_owned()),
            "discovery should have found whisker's own source: {files:?}"
        );
        let fixtures: Vec<&String> = files
            .iter()
            .filter(|file| {
                file.starts_with("examples/")
                    || file.starts_with("crates/whisker-rust/tests/fixtures/")
                    || (file.starts_with("lints/") && file.contains("/ui/"))
            })
            .collect();
        assert!(
            fixtures.is_empty(),
            "the configured patterns should have excluded every fixture project: {fixtures:?}"
        );
    }

    /// Pins the sort that makes a run's diagnostics reproducible
    ///
    /// Every other ordered assertion here is satisfied by the filesystem
    /// rather than by whisker: they name two or three files, and two or three
    /// files come back sorted often enough that removing the sort breaks
    /// nothing. Diagnostics arriving in a different order on every machine is
    /// a real cost to anyone diffing two runs, so this uses enough names for
    /// the storage order to be visibly its own, and says so out loud if that
    /// stops being true.
    #[test]
    fn run_returns_files_in_sorted_order() {
        let directory = tree(&[
            "zebra.rs",
            "yak.rs",
            "xray.rs",
            "walrus.rs",
            "viper.rs",
            "umbrella.rs",
            "tiger.rs",
            "snake.rs",
        ]);
        assert_stored_out_of_order(directory.path());
        let config = config(directory.path(), &[]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(
            discovered(&discovery, directory.path()),
            [
                "snake.rs",
                "tiger.rs",
                "umbrella.rs",
                "viper.rs",
                "walrus.rs",
                "xray.rs",
                "yak.rs",
                "zebra.rs"
            ]
        );
    }

    // r[verify cli.config.ignore]
    #[test]
    fn run_with_anchored_pattern_prunes_only_at_the_workspace_root() {
        let directory = tree(&[
            "examples/demo.rs",
            "crates/app/examples/demo.rs",
            "src/main.rs",
        ]);
        let config = config(directory.path(), &["/examples/"]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(
            discovered(&discovery, directory.path()),
            ["crates/app/examples/demo.rs", "src/main.rs"]
        );
    }

    #[test]
    fn run_with_directory_pattern_prunes_the_directory() {
        let directory = tree(&["src/main.rs", "examples/demo/src/main.rs"]);
        let config = config(directory.path(), &["examples/"]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    // r[verify cli.discovery.ignore-files]
    #[test]
    fn run_with_dot_ignore_file_excludes_it() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(directory.path().join(".ignore"), "generated/\n")
            .expect("ignore file should be written");
        let config = config(directory.path(), &[]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_excluded_directory_does_not_reinclude_a_file_inside_it() {
        let directory = tree(&["examples/demo.rs", "examples/keep.rs", "src/main.rs"]);
        let config = config(directory.path(), &["examples/", "!examples/keep.rs"]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    // r[verify cli.discovery.explicit-target]
    #[test]
    fn run_with_explicit_directory_target_is_not_pruned() {
        let directory = tree(&["examples/demo.rs"]);
        let config = config(directory.path(), &["examples/"]);
        let target = directory.path().join("examples");

        let discovery = Discovery::run(&target, &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, &target), ["demo.rs"]);
    }

    #[test]
    fn run_with_explicit_extensionless_file_target_returns_error() {
        let directory = tree(&["LICENSE"]);
        let config = config(directory.path(), &[]);
        let target = directory.path().join("LICENSE");

        let error = Discovery::run(&target, &config, WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("no file extension"),
            "error should explain that the language cannot be told: {error:#}"
        );
    }

    // r[verify cli.discovery.explicit-target]
    #[test]
    fn run_with_explicit_file_target_ignores_patterns() {
        let directory = tree(&["examples/demo.rs"]);
        let config = config(directory.path(), &["examples/", "*.rs"]);
        let target = directory.path().join("examples").join("demo.rs");

        let discovery = Discovery::run(&target, &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovery.files(), vec![target]);
    }

    #[test]
    fn run_with_explicit_non_rust_file_target_returns_error() {
        let directory = tree(&["Cargo.toml"]);
        let config = config(directory.path(), &[]);
        let target = directory.path().join("Cargo.toml");

        let error = Discovery::run(&target, &config, WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("no grammar for `.toml` files"),
            "error should name the extension whisker cannot parse: {error:#}"
        );
    }

    /// Pins the one exclusion whose rules live outside the tree they describe
    ///
    /// A `.git/info/exclude` is how a checkout hides something without saying
    /// so in a tracked file, and the walker only reads it after recognizing
    /// the directory as a repository. That recognition is the fragile part:
    /// an empty `.git` directory is enough here, but the moment whisker
    /// reconfigures the walk it is also the first of the four ignore sources
    /// to fall out silently.
    // r[verify cli.discovery.ignore-files]
    #[test]
    fn run_with_git_exclude_file_excludes_it() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::create_dir_all(directory.path().join(".git").join("info"))
            .expect("git directory should be created");
        std::fs::write(
            directory.path().join(".git").join("info").join("exclude"),
            "generated/\n",
        )
        .expect("exclude file should be written");
        let config = config(directory.path(), &[]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    // r[verify cli.discovery.ignore-files]
    #[test]
    fn run_with_gitignored_file_excludes_it() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "generated/\n")
            .expect("gitignore should be written");
        let config = config(directory.path(), &[]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    // r[verify cli.discovery.ignore-files]
    #[test]
    fn run_with_hidden_directory_excludes_it() {
        let directory = tree(&["src/main.rs", ".cache/build.rs"]);
        let config = config(directory.path(), &[]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_negated_pattern_reincludes_the_file() {
        let directory = tree(&["examples/demo.rs", "examples/keep.rs", "src/main.rs"]);
        let config = config(directory.path(), &["examples/*.rs", "!examples/keep.rs"]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(
            discovered(&discovery, directory.path()),
            ["examples/keep.rs", "src/main.rs"]
        );
    }

    #[test]
    fn run_with_non_rust_files_excludes_them() {
        let directory = tree(&["src/main.rs", "README.md", "Cargo.toml", "LICENSE"]);
        let config = config(directory.path(), &[]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_nonexistent_path_returns_error() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let config = config(directory.path(), &[]);

        let error = Discovery::run(
            &directory.path().join("missing"),
            &config,
            WalkErrorPolicy::Fail,
        )
        .expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("failed to resolve"),
            "error should report the unresolvable path: {error:#}"
        );
    }

    #[test]
    fn run_with_pattern_matching_nothing_keeps_every_file() {
        let directory = tree(&["src/main.rs", "src/lib.rs"]);
        let config = config(directory.path(), &["vendor/", "*.py"]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(
            discovered(&discovery, directory.path()),
            ["src/lib.rs", "src/main.rs"]
        );
        assert!(discovery.errors().is_empty());
    }

    #[test]
    fn run_with_pattern_outside_the_walk_root_still_anchors_at_the_workspace() {
        let directory = tree(&["crates/app/src/main.rs", "crates/app/src/generated.rs"]);
        let config = config(directory.path(), &["crates/app/src/generated.rs"]);
        let target = directory.path().join("crates").join("app");

        let discovery = Discovery::run(&target, &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, &target), ["src/main.rs"]);
    }

    #[cfg(unix)]
    #[test]
    fn run_with_target_reached_through_a_symlink_returns_the_given_prefix() {
        let directory = tree(&["src/main.rs"]);
        let config = config(directory.path(), &[]);
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(directory.path().join("src"), &link)
            .expect("symlink should be created");

        let discovery = Discovery::run(&link, &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovery.files(), vec![link.join("main.rs")]);
    }

    // r[verify cli.config.ignore]
    #[test]
    fn run_with_unanchored_pattern_prunes_at_every_depth() {
        let directory = tree(&[
            "examples/demo.rs",
            "crates/app/examples/demo.rs",
            "src/main.rs",
        ]);
        let config = config(directory.path(), &["examples/"]);

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    /// Pins that an ignore file is named as one wherever the walker found it
    ///
    /// The rules a directory's ancestors impose are collected before the walk
    /// has any entry to hang a failure on, so this fault takes the walker's
    /// generic error path rather than the one every other unparsable ignore
    /// file takes. The user has the same file to fix either way, and being
    /// told to go looking at directory entries instead is worse than being
    /// told nothing.
    // r[verify cli.discovery.walk-errors]
    #[test]
    fn run_with_unparsable_ignore_file_above_the_root_returns_error() {
        let directory = tree(&["proj/src/main.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "{a,b\n")
            .expect("gitignore should be written");
        let config = config(directory.path(), &[]);
        let target = directory.path().join("proj");

        let error = Discovery::run(&target, &config, WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

        let error = format!("{error:#}");
        assert!(
            error.contains("failed to read an ignore file"),
            "error should describe the unparsable ignore file: {error}"
        );
        assert!(
            !error.contains("failed to read a directory entry"),
            "error should not blame the directory walk: {error}"
        );
    }

    // r[verify cli.discovery.walk-errors]
    #[test]
    fn run_with_unparsable_ignore_file_and_fail_policy_returns_error() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(
            directory.path().join("generated").join(".gitignore"),
            "{a,b\n",
        )
        .expect("gitignore should be written");
        let config = config(directory.path(), &[]);

        let error = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("failed to read an ignore file"),
            "error should describe the unparsable ignore file: {error:#}"
        );
    }

    // r[verify cli.discovery.walk-errors]
    #[test]
    fn run_with_unparsable_ignore_file_and_keep_going_records_the_error() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(
            directory.path().join("generated").join(".gitignore"),
            "{a,b\n",
        )
        .expect("gitignore should be written");
        let config = config(directory.path(), &[]);

        let discovery = Discovery::run(
            directory.path(),
            &config,
            WalkErrorPolicy::ReportAndContinue,
        )
        .expect("discovery should succeed");

        assert_eq!(
            discovered(&discovery, directory.path()),
            ["generated/schema.rs", "src/main.rs"]
        );
        assert_eq!(discovery.errors().len(), 1);
    }

    // r[verify cli.discovery.walk-errors]
    #[test]
    fn run_with_unparsable_root_ignore_file_returns_error() {
        let directory = tree(&["src/main.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "{a,b\n")
            .expect("gitignore should be written");
        let config = config(directory.path(), &[]);

        let error = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("failed to read an ignore file"),
            "error should describe the unparsable ignore file: {error:#}"
        );
    }

    // r[verify cli.discovery.walk-errors]
    #[cfg(unix)]
    #[test]
    fn run_with_unreadable_directory_and_fail_policy_returns_error() {
        let directory = tree(&["src/main.rs", "locked/inner.rs"]);
        let config = config(directory.path(), &[]);
        let locked = directory.path().join("locked");
        let _unreadable = make_unreadable(&locked);

        let error = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("failed to read a directory entry"),
            "error should describe the unreadable entry: {error:#}"
        );
    }

    // r[verify cli.discovery.walk-errors]
    #[cfg(unix)]
    #[test]
    fn run_with_unreadable_directory_and_keep_going_records_the_error() {
        let directory = tree(&["src/main.rs", "locked/inner.rs"]);
        let config = config(directory.path(), &[]);
        let locked = directory.path().join("locked");
        let _unreadable = make_unreadable(&locked);

        let discovery = Discovery::run(
            directory.path(),
            &config,
            WalkErrorPolicy::ReportAndContinue,
        )
        .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
        assert_eq!(discovery.errors().len(), 1);
    }

    // r[verify cli.config.ignore]
    #[test]
    fn run_with_workspace_configuration_excludes_the_configured_pattern() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(
            directory.path().join("Cargo.toml"),
            format!("{EMPTY_WORKSPACE}\n[workspace.metadata.whisker]\nignore = [\"generated/\"]\n"),
        )
        .expect("manifest should be written");
        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        let discovery = Discovery::run(directory.path(), &config, WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Discovery>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Discovery>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Discovery>();
    }
}
