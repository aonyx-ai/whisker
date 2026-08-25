use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Deserialize;

mod git_rev;
mod git_url;
mod ignore_pattern;
mod lint_path;
mod lint_source;

pub use git_rev::GitRev;
pub use git_url::GitUrl;
pub use ignore_pattern::IgnorePattern;
pub use lint_path::LintPath;
pub use lint_source::{GitLintSource, LintSource};

/// Whisker's configuration for a single target project
///
/// Whisker reads a TOML file that any project can write, whatever language
/// it is in. The file holds the [`IgnorePattern`]s that exclude files from
/// discovery and the [`LintSource`]s of the project's custom lints. Both
/// anchor at the project directory, which [`WhiskerConfig::root`] returns.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct WhiskerConfig {
    root: PathBuf,
    ignore: Vec<IgnorePattern>,
    lints: Vec<LintSource>,
}

impl WhiskerConfig {
    /// Creates a configuration that anchors its patterns at `root`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = WhiskerConfig::new(
    ///     PathBuf::from("/src/project"),
    ///     vec![IgnorePattern::new("examples/")],
    ///     Vec::new(),
    /// );
    /// ```
    pub fn new(root: PathBuf, ignore: Vec<IgnorePattern>, lints: Vec<LintSource>) -> Self {
        Self {
            root,
            ignore,
            lints,
        }
    }

    /// Loads the configuration that governs `path`
    ///
    /// Whisker accepts `.whisker.toml` and `.config/whisker.toml`. The
    /// search starts at the directory for `path` and climbs, so a run on a
    /// subdirectory still reads the project's configuration. The climb stops
    /// at the directory that holds `.git`. A project with no configuration
    /// file is a supported target, and its patterns are empty.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` cannot be resolved, or if one directory
    /// holds both accepted file names. A file that whisker cannot read as an
    /// `ignore` list of strings is also an error. So is a `[[lints]]` entry
    /// that does not describe exactly one source. An unrecognized key is an
    /// error too, because it is most likely a typo.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = WhiskerConfig::load(Path::new("."))?;
    ///
    /// println!("{} patterns", config.ignore().len());
    /// ```
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let start = search_directory(path)?;

        let Some(FoundConfig { root, file }) = find_config(&start)? else {
            return Ok(Self::new(start, Vec::new(), Vec::new()));
        };

        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("failed to read {}", file.display()))?;

        let ConfigTable { ignore, lints } =
            toml::from_str(&text).with_context(|| format!("failed to read {}", file.display()))?;

        let ignore = ignore.into_iter().map(IgnorePattern::new).collect();
        let lints = lints
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                entry
                    .into_source()
                    .with_context(|| format!("failed to read [[lints]] entry {}", index + 1))
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .with_context(|| format!("failed to read {}", file.display()))?;

        Ok(Self::new(root, ignore, lints))
    }

    /// Returns the project directory that anchors the ignore patterns
    ///
    /// This is the directory that holds `.whisker.toml`, or the directory
    /// that holds `.config`. Without a configuration file, it is the
    /// directory the search started from.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = WhiskerConfig::load(Path::new("."))?;
    ///
    /// println!("configured from {}", config.root().display());
    /// ```
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the patterns that exclude files from discovery
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = WhiskerConfig::load(Path::new("."))?;
    ///
    /// assert!(config.ignore().is_empty());
    /// ```
    pub fn ignore(&self) -> &[IgnorePattern] {
        &self.ignore
    }

    /// Returns the sources of the project's custom lint crates
    ///
    /// A [`LintSource::Path`] holding a relative path anchors at
    /// [`WhiskerConfig::root`]; resolve it with [`LintPath::resolve`].
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = WhiskerConfig::load(Path::new("."))?;
    ///
    /// assert!(config.lints().is_empty());
    /// ```
    pub fn lints(&self) -> &[LintSource] {
        &self.lints
    }
}

/// The configuration file as written on disk
///
/// [`WhiskerConfig::load`] turns the strings here into [`IgnorePattern`]s
/// and [`LintPath`]s. Whisker rejects a key it does not recognize, because
/// a key it silently drops looks exactly like one that works.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigTable {
    #[serde(default)]
    ignore: Vec<String>,

    #[serde(default)]
    lints: Vec<LintEntry>,
}

/// One `[[lints]]` entry as written on disk
///
/// The fields are all optional here, and [`LintEntry::into_source`] sorts
/// them out rather than an untagged enum. Serde reports a failed untagged
/// match as "data did not match any variant". The author of a broken entry
/// deserves to be told which combination they wrote, and what to do about
/// it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LintEntry {
    #[serde(default)]
    path: Option<String>,

    #[serde(default)]
    git: Option<String>,

    #[serde(default)]
    rev: Option<String>,
}

impl LintEntry {
    /// Turns one configured entry into the source it describes
    ///
    /// # Errors
    ///
    /// Returns an error if the entry names neither a path nor a repository,
    /// or names both. Returns one too if the entry omits the revision a
    /// repository needs, or pins a path to a revision.
    ///
    /// Each error names the keys at fault and nothing else.
    /// [`WhiskerConfig::load`] is the caller that knows which entry the
    /// reader has to find.
    fn into_source(self) -> anyhow::Result<LintSource> {
        let Self { path, git, rev } = self;

        match (path, git, rev) {
            (Some(_), Some(_), _) => {
                anyhow::bail!("can only define either path or git, not both")
            }
            (Some(_), None, Some(_)) => {
                anyhow::bail!("can only define rev with git, not with path")
            }
            (Some(path), None, None) => Ok(LintSource::Path(LintPath::new(path))),
            (None, Some(git), Some(rev)) => {
                let url = GitUrl::new(git)?;
                let rev = GitRev::new(rev)?;

                Ok(LintSource::Git(GitLintSource::new(url, rev)))
            }
            (None, Some(_), None) => anyhow::bail!("must define rev with git"),
            (None, None, Some(_)) => anyhow::bail!("must define git with rev"),
            (None, None, None) => anyhow::bail!("must define either path, or git and rev"),
        }
    }
}

/// A configuration file whisker found, and the directory it anchors to
///
/// The two differ for `.config/whisker.toml`, where the patterns anchor to
/// the project directory rather than to `.config`.
#[derive(Clone, Eq, PartialEq, Debug)]
struct FoundConfig {
    root: PathBuf,
    file: PathBuf,
}

/// Returns the configuration file that governs `start`, if there is one
///
/// The search climbs from `start` and stops after the directory that holds
/// `.git`. A run therefore never reads configuration from outside the
/// repository it targets. Without a repository, the search climbs to the
/// filesystem root.
///
/// # Errors
///
/// Returns an error if one directory holds both accepted file names,
/// because whisker cannot tell which one the author meant.
fn find_config(start: &Path) -> anyhow::Result<Option<FoundConfig>> {
    for directory in start.ancestors() {
        let dotfile = directory.join(".whisker.toml");
        let nested = directory.join(".config").join("whisker.toml");

        match (dotfile.is_file(), nested.is_file()) {
            (true, true) => anyhow::bail!(
                "whisker found two configuration files in {}:\n  {}\n  {}\nkeep one and delete \
                 the other",
                directory.display(),
                dotfile.display(),
                nested.display()
            ),
            (true, false) => {
                return Ok(Some(FoundConfig {
                    root: directory.to_path_buf(),
                    file: dotfile,
                }));
            }
            (false, true) => {
                return Ok(Some(FoundConfig {
                    root: directory.to_path_buf(),
                    file: nested,
                }));
            }
            (false, false) => {}
        }

        if directory.join(".git").exists() {
            break;
        }
    }

    Ok(None)
}

/// Returns the absolute directory where the search for `path` starts
///
/// For a file target, this is the directory that holds the file.
///
/// # Errors
///
/// Returns an error if `path` does not exist or cannot be resolved.
fn search_directory(path: &Path) -> anyhow::Result<PathBuf> {
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("failed to resolve {}", path.display()))?;

    if path.is_dir() {
        return Ok(path);
    }

    let directory = path.parent().unwrap_or(&path).to_path_buf();

    Ok(directory)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// A commit hash of the right shape for a configuration under test
    const REV: &str = "0123456789abcdef0123456789abcdef01234567";

    /// Returns the resolved path of a temporary directory
    ///
    /// Temporary directories commonly sit behind a symlink, and the search
    /// resolves its starting point, so comparisons have to resolve too.
    fn resolved(directory: &TempDir) -> PathBuf {
        std::fs::canonicalize(directory.path()).expect("temporary directory should resolve")
    }

    /// Creates a temporary directory that looks like a repository root
    ///
    /// The `.git` entry stops the upward search, which keeps a stray
    /// configuration file above the temporary directory out of every test.
    fn repository() -> TempDir {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::create_dir(directory.path().join(".git"))
            .expect("git directory should be created");
        directory
    }

    /// Writes `contents` to `.whisker.toml` in `directory`
    fn write_dotfile(directory: &Path, contents: &str) {
        std::fs::write(directory.join(".whisker.toml"), contents)
            .expect("configuration should be written");
    }

    /// Writes `contents` to `.config/whisker.toml` in `directory`
    fn write_nested(directory: &Path, contents: &str) {
        let config = directory.join(".config");
        std::fs::create_dir_all(&config).expect("config directory should be created");
        std::fs::write(config.join("whisker.toml"), contents)
            .expect("configuration should be written");
    }

    /// Pins that an error names which entry the reader has to go and fix
    ///
    /// The keys of a broken entry rarely tell it apart from the entry
    /// above it, and a repository may configure several. The position in
    /// the file is what always distinguishes them.
    #[test]
    fn load_with_a_broken_second_lint_entry_names_its_position() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            "[[lints]]\npath = \"lints/first\"\n\n[[lints]]\npath = \"lints/second\"\ngit = \"https://example.com/rules\"\n",
        );

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("[[lints]] entry 2"),
            "error should name the entry at fault: {error:#}"
        );
    }

    #[test]
    fn load_with_both_config_files_returns_error() {
        let directory = repository();
        write_dotfile(directory.path(), "ignore = []\n");
        write_nested(directory.path(), "ignore = []\n");

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        let message = format!("{error:#}");
        assert!(
            message.contains(".whisker.toml") && message.contains("whisker.toml"),
            "error should name both files: {message}"
        );
        assert!(
            message.contains("two configuration files"),
            "error should explain the conflict: {message}"
        );
    }

    #[test]
    fn load_with_config_above_the_target_walks_up() {
        let directory = repository();
        write_dotfile(directory.path(), "ignore = [\"examples/\"]\n");
        let nested = directory.path().join("src").join("inner");
        std::fs::create_dir_all(&nested).expect("directories should be created");

        let config = WhiskerConfig::load(&nested).expect("configuration should load");

        assert_eq!(config.root(), resolved(&directory));
        assert_eq!(config.ignore(), vec![IgnorePattern::new("examples/")]);
    }

    #[test]
    fn load_with_config_outside_the_repository_is_not_read() {
        let outer = tempfile::tempdir().expect("temporary directory should be created");
        write_dotfile(outer.path(), "ignore = [\"examples/\"]\n");
        let inner = outer.path().join("project");
        std::fs::create_dir(&inner).expect("project should be created");
        std::fs::create_dir(inner.join(".git")).expect("git directory should be created");

        let config = WhiskerConfig::load(&inner).expect("configuration should load");

        assert!(
            config.ignore().is_empty(),
            "the search must stop at the repository root"
        );
    }

    #[test]
    fn load_with_empty_ignore_list_returns_an_empty_configuration() {
        let directory = repository();
        write_dotfile(directory.path(), "ignore = []\n");

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert!(config.ignore().is_empty());
    }

    #[test]
    fn load_with_file_target_uses_the_config_root() {
        let directory = repository();
        write_dotfile(directory.path(), "ignore = [\"examples/\"]\n");
        std::fs::create_dir(directory.path().join("src")).expect("src should be created");
        let file = directory.path().join("src").join("main.rs");
        std::fs::write(&file, "fn main() {}").expect("source should be written");

        let config = WhiskerConfig::load(&file).expect("configuration should load");

        assert_eq!(config.root(), resolved(&directory));
        assert_eq!(config.ignore(), vec![IgnorePattern::new("examples/")]);
    }

    #[test]
    fn load_with_git_lint_entry_holding_a_branch_name_returns_error() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            "[[lints]]\ngit = \"https://example.com/rules\"\nrev = \"main\"\n",
        );

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("40 characters"),
            "error should explain the pin: {error:#}"
        );
        assert!(
            format!("{error:#}").contains("[[lints]] entry 1"),
            "error should name the entry it read: {error:#}"
        );
    }

    #[test]
    fn load_with_git_lint_entry_holding_an_abbreviated_rev_returns_error() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            "[[lints]]\ngit = \"https://example.com/rules\"\nrev = \"0123456\"\n",
        );

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("40 characters"),
            "error should explain the pin: {error:#}"
        );
    }

    /// Pins that a token in a remote never reaches an error message
    ///
    /// A remote is where a token sits when someone pins a private rule
    /// repository, and stderr becomes a CI log. Reading a bad entry is the
    /// first thing whisker does with a remote, so it is the first place a
    /// token could escape.
    #[test]
    fn load_with_git_lint_entry_holding_credentials_hides_them() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            "[[lints]]\ngit = \"https://user:s3cret@example.com/rules\"\nrev = \"main\"\n",
        );

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            !format!("{error:#}").contains("s3cret"),
            "error should not carry the credentials: {error:#}"
        );
    }

    #[test]
    fn load_with_git_lint_entry_missing_rev_returns_error() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            "[[lints]]\ngit = \"https://example.com/rules\"\n",
        );

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("must define rev with git"),
            "error should name the missing key: {error:#}"
        );
        assert!(
            format!("{error:#}").contains("[[lints]] entry 1"),
            "error should name the entry it read: {error:#}"
        );
    }

    #[test]
    fn load_with_git_lint_entry_returns_a_git_source() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            &format!("[[lints]]\ngit = \"https://example.com/rules\"\nrev = \"{REV}\"\n"),
        );

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert_eq!(
            config.lints(),
            vec![LintSource::Git(GitLintSource::new(
                GitUrl::new("https://example.com/rules").expect("the remote should be accepted"),
                GitRev::new(REV).expect("the revision should be accepted"),
            ))]
        );
    }

    #[test]
    fn load_with_ignore_patterns_returns_them_in_order() {
        let directory = repository();
        write_dotfile(directory.path(), "ignore = [\"examples/\", \"a/b.rs\"]\n");

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert_eq!(
            config.ignore(),
            vec![
                IgnorePattern::new("examples/"),
                IgnorePattern::new("a/b.rs")
            ]
        );
    }

    #[test]
    fn load_with_invalid_toml_returns_error() {
        let directory = repository();
        write_dotfile(directory.path(), "ignore = [\nthis is not toml\n");

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains(".whisker.toml"),
            "error should name the file it could not read: {error:#}"
        );
    }

    #[test]
    fn load_with_lint_entries_returns_them_in_order() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            "[[lints]]\npath = \"lints/no_todo\"\n\n[[lints]]\npath = \"lints/prefer_expect\"\n",
        );

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert_eq!(
            config.lints(),
            vec![
                LintSource::Path(LintPath::new("lints/no_todo")),
                LintSource::Path(LintPath::new("lints/prefer_expect")),
            ]
        );
    }

    #[test]
    fn load_with_lint_entry_holding_git_and_path_returns_error() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            &format!(
                "[[lints]]\npath = \"lints/no_todo\"\ngit = \"https://example.com/rules\"\nrev = \
                 \"{REV}\"\n"
            ),
        );

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("can only define either path or git"),
            "error should name the conflict: {error:#}"
        );
    }

    #[test]
    fn load_with_lint_entry_holding_neither_path_nor_git_returns_error() {
        let directory = repository();
        write_dotfile(directory.path(), "[[lints]]\n");

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("must define either path, or git and rev"),
            "error should explain what an entry needs: {error:#}"
        );
    }

    #[test]
    fn load_with_lint_entry_holding_rev_without_git_returns_error() {
        let directory = repository();
        write_dotfile(directory.path(), &format!("[[lints]]\nrev = \"{REV}\"\n"));

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("must define git with rev"),
            "error should name the missing key: {error:#}"
        );
    }

    #[test]
    fn load_with_lint_entry_missing_path_returns_error() {
        let directory = repository();
        write_dotfile(directory.path(), "[[lints]]\nname = \"no_todo\"\n");

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains(".whisker.toml"),
            "error should name the offending file: {error:#}"
        );
    }

    #[test]
    fn load_with_lint_entry_pinning_a_path_returns_error() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            &format!("[[lints]]\npath = \"lints/no_todo\"\nrev = \"{REV}\"\n"),
        );

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("can only define rev with git"),
            "error should name the conflict: {error:#}"
        );
    }

    #[test]
    fn load_with_lint_entry_unknown_key_returns_error() {
        let directory = repository();
        write_dotfile(
            directory.path(),
            "[[lints]]\npath = \"lints/no_todo\"\nprofile = \"release\"\n",
        );

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("profile"),
            "error should name the key whisker does not recognize: {error:#}"
        );
    }

    #[test]
    fn load_with_malformed_ignore_value_returns_error() {
        let directory = repository();
        write_dotfile(directory.path(), "ignore = \"examples/\"\n");

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains(".whisker.toml"),
            "error should name the offending file: {error:#}"
        );
        assert!(
            format!("{error:#}").contains("invalid type"),
            "error should describe the type mismatch: {error:#}"
        );
    }

    #[test]
    fn load_with_nested_config_file_anchors_at_the_project_root() {
        let directory = repository();
        write_nested(directory.path(), "ignore = [\"examples/\"]\n");

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert_eq!(
            config.root(),
            resolved(&directory),
            "patterns must anchor beside .config, not inside it"
        );
        assert_eq!(config.ignore(), vec![IgnorePattern::new("examples/")]);
    }

    #[test]
    fn load_with_no_config_file_returns_an_empty_configuration() {
        let directory = repository();

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert_eq!(config.root(), resolved(&directory));
        assert!(config.ignore().is_empty());
        assert!(config.lints().is_empty());
    }

    #[test]
    fn load_with_unknown_key_returns_error() {
        let directory = repository();
        write_dotfile(directory.path(), "ignore = []\nexclude = [\"examples/\"]\n");

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("exclude"),
            "error should name the key whisker does not recognize: {error:#}"
        );
    }

    #[test]
    fn load_without_lint_entries_returns_no_lints() {
        let directory = repository();
        write_dotfile(directory.path(), "ignore = []\n");

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert!(config.lints().is_empty());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WhiskerConfig>();
        assert_send::<FoundConfig>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<WhiskerConfig>();
        assert_sync::<FoundConfig>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<WhiskerConfig>();
        assert_unpin::<FoundConfig>();
    }
}
