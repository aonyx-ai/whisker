use std::path::{Path, PathBuf};

use anyhow::Context as _;
use ignore::WalkBuilder;
use whisker_types::Language;

/// The source files a `whisker check` run inspects
///
/// The walk skips hidden files and directories, and it honors `.gitignore`,
/// `.ignore`, `.git/info/exclude`, and the user's global gitignore. Unlike
/// `ripgrep`, it applies these rules even without a repository. An ignore
/// file in a tarball or a vendored tree still describes which files that
/// tree generates.
#[derive(Debug)]
pub struct Discovery {
    files: Vec<PathBuf>,
}

impl Discovery {
    /// Discovers the files to lint beneath `path`
    ///
    /// A `path` that names one file comes back as-is. Ignore rules do not
    /// apply to it, because the user already chose the file.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` cannot be resolved.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let discovery = Discovery::run(Path::new("."))?;
    ///
    /// for file in discovery.files() {
    ///     println!("{}", file.display());
    /// }
    /// ```
    pub fn run(path: &Path) -> anyhow::Result<Self> {
        let root = std::fs::canonicalize(path)
            .with_context(|| format!("failed to resolve {}", path.display()))?;

        if root.is_file() {
            return Ok(Self {
                files: vec![path.to_path_buf()],
            });
        }

        let walk = WalkBuilder::new(&root).require_git(false).build();

        let mut files = Vec::new();

        for entry in walk {
            let Ok(entry) = entry else {
                continue;
            };

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

        Ok(Self { files })
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
}

/// Rebases `entry` from the resolved `root` onto the `original` argument
///
/// The walk resolves its root, but diagnostics must show the path the user
/// typed. `whisker check .` reports `./src/main.rs`, not the resolved path.
fn rebase(original: &Path, root: &Path, entry: &Path) -> PathBuf {
    original.join(entry.strip_prefix(root).unwrap_or(entry))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// Panics unless `directory` reads back in unsorted order
    ///
    /// A short file list often comes back sorted by accident, and a sort
    /// assertion over such a list proves nothing.
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

    /// Creates a temporary directory that holds the given relative files
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

    /// The sort makes diagnostics reproducible across machines. This test
    /// uses enough files for the storage order to differ from the sorted
    /// order, so it fails without the sort.
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

        let discovery = Discovery::run(directory.path()).expect("discovery should succeed");

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

    #[test]
    fn run_with_dot_ignore_file_excludes_it() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(directory.path().join(".ignore"), "generated/\n")
            .expect("ignore file should be written");

        let discovery = Discovery::run(directory.path()).expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    /// An ignore rule that matches a named directory does not prune it. The
    /// walker reads the ignore files above its root, but it never applies
    /// them to the root.
    #[test]
    fn run_with_explicit_directory_target_is_not_pruned() {
        let directory = tree(&["examples/demo.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "examples/\n")
            .expect("gitignore should be written");
        let target = directory.path().join("examples");

        let discovery = Discovery::run(&target).expect("discovery should succeed");

        assert_eq!(discovered(&discovery, &target), ["demo.rs"]);
    }

    /// A file the user names skips the walk, so no ignore rule reaches it
    #[test]
    fn run_with_explicit_file_target_is_not_ignored() {
        let directory = tree(&["generated/schema.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "generated/\n")
            .expect("gitignore should be written");
        let target = directory.path().join("generated").join("schema.rs");

        let discovery = Discovery::run(&target).expect("discovery should succeed");

        assert_eq!(
            discovered(&discovery, directory.path()),
            ["generated/schema.rs"]
        );
    }

    /// The walker reads `.git/info/exclude` only after it recognizes the
    /// directory as a repository. An empty `.git` directory is enough.
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

        let discovery = Discovery::run(directory.path()).expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_gitignored_file_excludes_it() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "generated/\n")
            .expect("gitignore should be written");

        let discovery = Discovery::run(directory.path()).expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_hidden_directory_excludes_it() {
        let directory = tree(&["src/main.rs", ".cache/build.rs"]);

        let discovery = Discovery::run(directory.path()).expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_non_rust_files_excludes_them() {
        let directory = tree(&["src/main.rs", "README.md", "Cargo.toml", "LICENSE"]);

        let discovery = Discovery::run(directory.path()).expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_nonexistent_path_returns_error() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        let error =
            Discovery::run(&directory.path().join("missing")).expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("failed to resolve"),
            "error should report the unresolvable path: {error:#}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_with_target_reached_through_a_symlink_returns_the_given_prefix() {
        let directory = tree(&["src/main.rs"]);
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(directory.path().join("src"), &link)
            .expect("symlink should be created");

        let discovery = Discovery::run(&link).expect("discovery should succeed");

        assert_eq!(discovery.files(), vec![link.join("main.rs")]);
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
