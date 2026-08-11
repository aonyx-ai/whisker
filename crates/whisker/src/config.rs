use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;

mod ignore_pattern;

pub use ignore_pattern::IgnorePattern;

/// Whisker's configuration for a single target project
///
/// Whisker reads its configuration from the `[workspace.metadata.whisker]`
/// table of the target project's workspace manifest, so there is no separate
/// configuration file. The table currently holds only the [`IgnorePattern`]s
/// that exclude files from discovery.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct WhiskerConfig {
    root: PathBuf,
    ignore: Vec<IgnorePattern>,
}

impl WhiskerConfig {
    /// Creates a configuration whose patterns are anchored at `root`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = WhiskerConfig::new(
    ///     PathBuf::from("/src/project"),
    ///     vec![IgnorePattern::new("examples/")],
    /// );
    /// ```
    pub fn new(root: PathBuf, ignore: Vec<IgnorePattern>) -> Self {
        Self { root, ignore }
    }

    /// Loads the configuration that governs `path`
    ///
    /// Whisker asks `cargo metadata` for the workspace root and reads the
    /// metadata table from its output. Cargo is the authority on the
    /// workspace root: it follows the `package.workspace` key and applies
    /// workspace inheritance. A directory outside any Cargo project is a
    /// supported target with an empty configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` cannot be resolved, if `cargo metadata`
    /// fails, or if the `[workspace.metadata.whisker]` table cannot be read.
    /// An unrecognized key in the table is an error, because it is most
    /// likely a typo.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let root = search_directory(path)?;

        if !has_manifest(&root) {
            return Ok(Self::new(root, Vec::new()));
        }

        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

        let output = Command::new(cargo)
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .current_dir(&root)
            .output()
            .context("failed to run `cargo metadata` to locate the workspace manifest")?;

        anyhow::ensure!(
            output.status.success(),
            "failed to read the Cargo workspace containing {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );

        let CargoMetadata {
            workspace_root,
            metadata,
        } = serde_json::from_slice(&output.stdout)
            .context("failed to parse the output of `cargo metadata`")?;

        let root = std::fs::canonicalize(&workspace_root).with_context(|| {
            format!(
                "failed to resolve the workspace root at {}",
                workspace_root.display()
            )
        })?;

        let Some(metadata) = metadata else {
            return Ok(Self::new(root, Vec::new()));
        };

        let Some(table) = metadata.get("whisker") else {
            return Ok(Self::new(root, Vec::new()));
        };

        let ConfigTable { ignore } = ConfigTable::deserialize(table).with_context(|| {
            format!(
                "failed to read the [workspace.metadata.whisker] table in {}",
                root.join("Cargo.toml").display()
            )
        })?;

        let ignore = ignore.into_iter().map(IgnorePattern::new).collect();

        Ok(Self::new(root, ignore))
    }

    /// Returns the directory that anchors the ignore patterns
    ///
    /// This is the Cargo workspace root when the target is part of a Cargo
    /// project, and the target directory itself otherwise.
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
}

/// The subset of `cargo metadata` output whisker reads
///
/// The workspace metadata stays an untyped map so that other tools' tables
/// pass through untouched. A malformed whisker table then produces an error
/// that names the whisker table, not the whole document.
#[derive(Debug, Deserialize)]
struct CargoMetadata {
    workspace_root: PathBuf,
    metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The `[workspace.metadata.whisker]` table as written in the manifest
///
/// The manifest is a system boundary, so this type holds plain strings.
/// [`WhiskerConfig::load`] turns them into [`IgnorePattern`]s.
/// Deserialization rejects unknown keys because a silently ignored key looks
/// exactly like one that works.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigTable {
    #[serde(default)]
    ignore: Vec<String>,
}

/// Returns whether `directory` or any of its ancestors holds a `Cargo.toml`
fn has_manifest(directory: &Path) -> bool {
    directory
        .ancestors()
        .any(|ancestor| ancestor.join("Cargo.toml").is_file())
}

/// Returns the absolute directory to ask Cargo about for `path`
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

    /// A virtual workspace manifest with no members
    ///
    /// Members would have to be built to be useful, and every test here is
    /// about the metadata table, not the crate graph.
    const EMPTY_WORKSPACE: &str = "[workspace]\nresolver = \"3\"\nmembers = []\n";

    /// Returns the resolved path of a temporary directory
    ///
    /// Temporary directories commonly sit behind a symlink, and Cargo reports
    /// the resolved workspace root, so comparisons have to resolve too.
    fn resolved(directory: &TempDir) -> PathBuf {
        std::fs::canonicalize(directory.path()).expect("temporary directory should resolve")
    }

    /// Creates a temporary Cargo workspace whose manifest is `manifest`
    fn workspace(manifest: &str) -> TempDir {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::write(directory.path().join("Cargo.toml"), manifest)
            .expect("manifest should be written");
        directory
    }

    #[test]
    fn load_with_empty_ignore_list_returns_an_empty_configuration() {
        let directory = workspace(&format!(
            "{EMPTY_WORKSPACE}\n[workspace.metadata.whisker]\nignore = []\n"
        ));

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert!(config.ignore().is_empty());
    }

    #[test]
    fn load_with_file_target_uses_the_workspace_root() {
        let directory = workspace(&format!(
            "{EMPTY_WORKSPACE}\n[workspace.metadata.whisker]\nignore = [\"examples/\"]\n"
        ));
        std::fs::create_dir(directory.path().join("src")).expect("src should be created");
        let file = directory.path().join("src").join("main.rs");
        std::fs::write(&file, "fn main() {}").expect("source should be written");

        let config = WhiskerConfig::load(&file).expect("configuration should load");

        assert_eq!(config.root(), resolved(&directory));
        assert_eq!(config.ignore(), vec![IgnorePattern::new("examples/")]);
    }

    #[test]
    fn load_with_ignore_patterns_returns_them_in_order() {
        let directory = workspace(&format!(
            "{EMPTY_WORKSPACE}\n[workspace.metadata.whisker]\nignore = [\"examples/\", \"a/b.rs\"]\n"
        ));

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
    fn load_with_invalid_manifest_returns_error() {
        let directory = workspace("[workspace\nthis is not toml\n");

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("failed to read the Cargo workspace containing"),
            "error should report that Cargo could not read the manifest: {error:#}"
        );
    }

    #[test]
    fn load_with_malformed_ignore_value_returns_error() {
        let directory = workspace(&format!(
            "{EMPTY_WORKSPACE}\n[workspace.metadata.whisker]\nignore = \"examples/\"\n"
        ));

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("[workspace.metadata.whisker]"),
            "error should name the offending table: {error:#}"
        );
        assert!(
            format!("{error:#}").contains("invalid type"),
            "error should describe the type mismatch: {error:#}"
        );
    }

    #[test]
    fn load_with_no_manifest_returns_an_empty_configuration() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert_eq!(config.root(), resolved(&directory));
        assert!(config.ignore().is_empty());
    }

    #[test]
    fn load_with_no_metadata_table_returns_an_empty_configuration() {
        let directory = workspace(EMPTY_WORKSPACE);

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert_eq!(config.root(), resolved(&directory));
        assert!(config.ignore().is_empty());
    }

    #[test]
    fn load_with_no_whisker_table_returns_an_empty_configuration() {
        let directory = workspace(&format!(
            "{EMPTY_WORKSPACE}\n[workspace.metadata.other-tool]\nsetting = true\n"
        ));

        let config = WhiskerConfig::load(directory.path()).expect("configuration should load");

        assert!(config.ignore().is_empty());
    }

    #[test]
    fn load_with_nonexistent_path_returns_error() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        let error = WhiskerConfig::load(&directory.path().join("missing"))
            .expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("failed to resolve"),
            "error should report the unresolvable path: {error:#}"
        );
    }

    #[test]
    fn load_with_unknown_key_returns_error() {
        let directory = workspace(&format!(
            "{EMPTY_WORKSPACE}\n[workspace.metadata.whisker]\nignoer = [\"examples/\"]\n"
        ));

        let error = WhiskerConfig::load(directory.path()).expect_err("configuration should fail");

        assert!(
            format!("{error:#}").contains("unknown field `ignoer`"),
            "error should name the unrecognized key: {error:#}"
        );
    }

    #[test]
    fn new_returns_the_given_root_and_patterns() {
        let root = PathBuf::from("/src/project");
        let patterns = vec![IgnorePattern::new("examples/")];

        let config = WhiskerConfig::new(root.clone(), patterns.clone());

        assert_eq!(config.root(), root);
        assert_eq!(config.ignore(), patterns);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WhiskerConfig>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<WhiskerConfig>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<WhiskerConfig>();
    }
}
