use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;

mod ignore_pattern;

pub use ignore_pattern::IgnorePattern;

/// Whisker's configuration for a single target project
///
/// Whisker is configured from the `[workspace.metadata.whisker]` table of the
/// target project's Cargo workspace manifest. Keeping configuration there
/// means there is no new file to discover, no new file format to learn, and no
/// question about which of several config files won: the workspace manifest is
/// already the one file every Cargo-aware tool agrees on.
///
/// The table is deliberately small. It carries only [`IgnorePattern`]s today,
/// and the richer configuration the specification anticipates - per-rule
/// severity and per-rule enablement - is meant to arrive as additional keys
/// beside `ignore`, each becoming another field on this type and another
/// accessor. Nothing about the shape of this type has to change to accommodate
/// them, and callers that only care about discovery keep reading
/// [`WhiskerConfig::ignore`] unchanged.
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
    /// Whisker asks Cargo where the workspace is instead of searching for a
    /// `[workspace]` table itself. `cargo metadata` is the authority on both
    /// the workspace root and its metadata table: it follows the
    /// `package.workspace` key that lets a member point at a root which is not
    /// one of its ancestors, it applies workspace inheritance, and it reports
    /// the same root that rust-analyzer will later load. A hand-rolled upward
    /// search would be a second, subtly different answer to a question Cargo
    /// already answers, and the two disagreeing is exactly the sort of bug
    /// that surfaces as "my ignore pattern does nothing".
    ///
    /// Whisker does perform its own upward search, but only to decide whether
    /// there is a manifest to ask Cargo about at all. A directory that is not
    /// part of any Cargo project is a supported target that simply has no
    /// configuration, rather than a Cargo error the user has to interpret.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` cannot be resolved, if `cargo metadata`
    /// cannot be run or reports a failure, or if the
    /// `[workspace.metadata.whisker]` table cannot be read - including when it
    /// contains a key whisker does not recognize, which is far more likely to
    /// be a typo than a deliberate annotation for some other tool.
    // r[impl cli.config.workspace-metadata]
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

        // r[impl cli.config.unknown-keys]
        let ConfigTable { ignore } = ConfigTable::deserialize(table).with_context(|| {
            format!(
                "failed to read the [workspace.metadata.whisker] table in {}",
                root.join("Cargo.toml").display()
            )
        })?;

        let ignore = ignore.into_iter().map(IgnorePattern::new).collect();

        Ok(Self::new(root, ignore))
    }

    /// Returns the directory the ignore patterns are resolved against
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

    /// Returns the patterns excluded from file discovery
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = WhiskerConfig::load(Path::new("."))?;
    ///
    /// assert!(config.ignore().is_empty());
    /// ```
    // r[impl cli.config.ignore]
    pub fn ignore(&self) -> &[IgnorePattern] {
        &self.ignore
    }
}

/// The subset of `cargo metadata` output whisker reads
///
/// The workspace metadata is kept as an untyped map rather than a struct so
/// that other tools' tables under `[workspace.metadata]` pass through
/// untouched, and so that a malformed whisker table produces an error naming
/// the whisker table rather than one naming the whole metadata document.
#[derive(Debug, Deserialize)]
struct CargoMetadata {
    workspace_root: PathBuf,
    metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The `[workspace.metadata.whisker]` table as written in the manifest
///
/// This is the wire shape, which is why it holds plain strings: the manifest
/// is a system boundary, and turning what it says into [`IgnorePattern`]s is
/// the job of [`WhiskerConfig::load`].
///
/// Unknown keys are rejected. A configuration key that whisker silently
/// ignores looks exactly like a configuration key that works, and the failure
/// mode - a project believing it excluded a directory that is still being
/// linted - is the kind of quiet wrongness this project exists to prevent.
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

/// Returns the absolute directory Cargo should be asked about for `path`
///
/// A file target is answered with the directory holding it, since a file has
/// no manifest of its own but the crate around it does.
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
    /// about the metadata table rather than the crate graph.
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

    // r[verify cli.config.ignore]
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

    // r[verify cli.config.workspace-metadata]
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

    // r[verify cli.config.unknown-keys]
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
