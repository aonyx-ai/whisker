use std::path::{Path, PathBuf};

use anyhow::Context as _;
use ignore::{DirEntry, WalkBuilder};
use whisker_types::Language;

mod walk_error_policy;
mod walk_failure;

pub use walk_error_policy::WalkErrorPolicy;
use walk_failure::WalkFailure;

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
    errors: Vec<anyhow::Error>,
}

impl Discovery {
    /// Discovers the files to lint beneath `path`
    ///
    /// A `path` that names one file comes back as-is. Ignore rules do not
    /// apply to it, because the user already chose the file. The grammar
    /// comes from the extension of `path`, not from the name a symlink
    /// resolves to. Whisker reports the path the user named.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` cannot be resolved, if `path` names a file
    /// with no extension, if `path` names a file written in a language
    /// whisker has no grammar for, if a configured
    /// ignore pattern is not valid gitignore syntax, or - under
    /// [`WalkErrorPolicy::Fail`] - if a directory entry or an ignore file
    /// cannot be read.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let discovery = Discovery::run(Path::new("."), WalkErrorPolicy::Fail)?;
    ///
    /// for file in discovery.files() {
    ///     println!("{}", file.display());
    /// }
    /// ```
    pub fn run(path: &Path, on_error: WalkErrorPolicy) -> anyhow::Result<Self> {
        let root = std::fs::canonicalize(path)
            .with_context(|| format!("failed to resolve {}", path.display()))?;

        if root.is_file() {
            let Some(extension) = path.extension() else {
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

        let walk = WalkBuilder::new(&root).require_git(false).build();

        let mut files = Vec::new();
        let mut errors = Vec::new();

        for entry in walk {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    let context = WalkFailure::classify(&error).context();
                    let error = anyhow::Error::new(error).context(context);

                    match on_error {
                        WalkErrorPolicy::Fail => return Err(error),
                        WalkErrorPolicy::ReportAndContinue => {
                            errors.push(error);
                            continue;
                        }
                    }
                }
            };

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
    /// The list is always empty under [`WalkErrorPolicy::Fail`], which
    /// returns the first failure instead of recording it.
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
/// An unparsable ignore file inside the tree does not abandon the walk. The
/// entry arrives intact with the failure attached, and the file's rules are
/// silently absent. Discovery treats this like an unreadable directory,
/// because both change the set of linted files.
fn attached_error(entry: &DirEntry) -> Option<anyhow::Error> {
    let error = entry.error()?;

    Some(anyhow::Error::msg(error.to_string()).context("failed to read an ignore file"))
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
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    use tempfile::TempDir;

    use super::*;

    /// A directory whose permissions are restored when this value is dropped
    ///
    /// A temporary directory left at mode `000` cannot be removed. This
    /// guard restores the mode even when a test panics, so [`TempDir`]'s
    /// cleanup still succeeds.
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

    /// Restores default permissions so the temporary directory can be removed
    #[cfg(unix)]
    fn make_readable(directory: &Path) {
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o755))
            .expect("permissions should be restored");
    }

    /// Makes `directory` impossible to read until the returned value is dropped
    ///
    /// A privileged process, root in a CI container for example, can still
    /// read a mode `000` directory. The walk-error tests would then assert
    /// nothing, so this panics instead of passing silently.
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

        let discovery = Discovery::run(directory.path(), WalkErrorPolicy::Fail)
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

    #[test]
    fn run_with_dot_ignore_file_excludes_it() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(directory.path().join(".ignore"), "generated/\n")
            .expect("ignore file should be written");

        let discovery = Discovery::run(directory.path(), WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

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

        let discovery =
            Discovery::run(&target, WalkErrorPolicy::Fail).expect("discovery should succeed");

        assert_eq!(discovered(&discovery, &target), ["demo.rs"]);
    }

    #[test]
    fn run_with_explicit_extensionless_file_target_returns_error() {
        let directory = tree(&["LICENSE"]);
        let target = directory.path().join("LICENSE");

        let error =
            Discovery::run(&target, WalkErrorPolicy::Fail).expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("no file extension"),
            "error should explain that the language cannot be told: {error:#}"
        );
    }

    /// Pins that a resolved `.rs` target does not rescue an extensionless name
    ///
    /// Whisker canonicalizes the path first, and canonicalization resolves
    /// the symlink. The resolved name must not pick the grammar, because the
    /// user named `LICENSE`.
    #[cfg(unix)]
    #[test]
    fn run_with_explicit_extensionless_symlink_to_a_rust_file_returns_error() {
        let directory = tree(&["src/main.rs"]);
        let link = directory.path().join("LICENSE");
        std::os::unix::fs::symlink(directory.path().join("src").join("main.rs"), &link)
            .expect("symlink should be created");

        let error =
            Discovery::run(&link, WalkErrorPolicy::Fail).expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("no file extension"),
            "error should explain that the language cannot be told: {error:#}"
        );
    }

    /// A file the user names skips the walk, so no ignore rule reaches it
    #[test]
    fn run_with_explicit_file_target_is_not_ignored() {
        let directory = tree(&["generated/schema.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "generated/\n")
            .expect("gitignore should be written");
        let target = directory.path().join("generated").join("schema.rs");

        let discovery =
            Discovery::run(&target, WalkErrorPolicy::Fail).expect("discovery should succeed");

        assert_eq!(
            discovered(&discovery, directory.path()),
            ["generated/schema.rs"]
        );
    }

    #[test]
    fn run_with_explicit_non_rust_file_target_returns_error() {
        let directory = tree(&["Cargo.toml"]);
        let target = directory.path().join("Cargo.toml");

        let error =
            Discovery::run(&target, WalkErrorPolicy::Fail).expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("no grammar for `.toml` files"),
            "error should name the extension whisker cannot parse: {error:#}"
        );
    }

    /// Pins that a resolved extensionless target does not reject a `.rs` name
    ///
    /// The user named `link.rs`, so `.rs` picks the grammar. Whisker reports
    /// the path the user named, not the resolved one.
    #[cfg(unix)]
    #[test]
    fn run_with_explicit_rust_symlink_to_an_extensionless_file_returns_the_link() {
        let directory = tree(&["data"]);
        let link = directory.path().join("link.rs");
        std::os::unix::fs::symlink(directory.path().join("data"), &link)
            .expect("symlink should be created");

        let discovery =
            Discovery::run(&link, WalkErrorPolicy::Fail).expect("discovery should succeed");

        assert_eq!(discovery.files(), vec![link]);
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

        let discovery = Discovery::run(directory.path(), WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_gitignored_file_excludes_it() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "generated/\n")
            .expect("gitignore should be written");

        let discovery = Discovery::run(directory.path(), WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_hidden_directory_excludes_it() {
        let directory = tree(&["src/main.rs", ".cache/build.rs"]);

        let discovery = Discovery::run(directory.path(), WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_non_rust_files_excludes_them() {
        let directory = tree(&["src/main.rs", "README.md", "Cargo.toml", "LICENSE"]);

        let discovery = Discovery::run(directory.path(), WalkErrorPolicy::Fail)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
    }

    #[test]
    fn run_with_nonexistent_path_returns_error() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        let error = Discovery::run(&directory.path().join("missing"), WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

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

        let discovery =
            Discovery::run(&link, WalkErrorPolicy::Fail).expect("discovery should succeed");

        assert_eq!(discovery.files(), vec![link.join("main.rs")]);
    }

    /// Pins that an ignore file is named as one wherever the walker found it
    ///
    /// An ignore file above the root fails before the walk has an entry, so
    /// it takes the walker's generic error path. The error must still name
    /// an ignore file, not a directory entry.
    #[test]
    fn run_with_unparsable_ignore_file_above_the_root_returns_error() {
        let directory = tree(&["proj/src/main.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "{a,b\n")
            .expect("gitignore should be written");
        let target = directory.path().join("proj");

        let error =
            Discovery::run(&target, WalkErrorPolicy::Fail).expect_err("discovery should fail");

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

    #[test]
    fn run_with_unparsable_ignore_file_and_fail_policy_returns_error() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(
            directory.path().join("generated").join(".gitignore"),
            "{a,b\n",
        )
        .expect("gitignore should be written");

        let error = Discovery::run(directory.path(), WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

        let error = format!("{error:#}");
        assert!(
            error.contains("failed to read an ignore file"),
            "error should describe the unparsable ignore file: {error}"
        );
        assert!(
            error.contains(".gitignore"),
            "error should name the file the walker could not parse: {error}"
        );
        assert!(
            error.contains("{a,b"),
            "error should quote the glob the walker could not parse: {error}"
        );
    }

    #[test]
    fn run_with_unparsable_ignore_file_and_keep_going_records_the_error() {
        let directory = tree(&["src/main.rs", "generated/schema.rs"]);
        std::fs::write(
            directory.path().join("generated").join(".gitignore"),
            "{a,b\n",
        )
        .expect("gitignore should be written");

        let discovery = Discovery::run(directory.path(), WalkErrorPolicy::ReportAndContinue)
            .expect("discovery should succeed");

        assert_eq!(
            discovered(&discovery, directory.path()),
            ["generated/schema.rs", "src/main.rs"]
        );
        assert_eq!(discovery.errors().len(), 1);
    }

    #[test]
    fn run_with_unparsable_root_ignore_file_returns_error() {
        let directory = tree(&["src/main.rs"]);
        std::fs::write(directory.path().join(".gitignore"), "{a,b\n")
            .expect("gitignore should be written");

        let error = Discovery::run(directory.path(), WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

        assert!(
            format!("{error:#}").contains("failed to read an ignore file"),
            "error should describe the unparsable ignore file: {error:#}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_with_unreadable_directory_and_fail_policy_returns_error() {
        let directory = tree(&["src/main.rs", "locked/inner.rs"]);
        let locked = directory.path().join("locked");
        let _unreadable = make_unreadable(&locked);

        let error = Discovery::run(directory.path(), WalkErrorPolicy::Fail)
            .expect_err("discovery should fail");

        let error = format!("{error:#}");
        assert!(
            error.contains("failed to read a directory entry"),
            "error should describe the unreadable entry: {error}"
        );
        assert!(
            error.contains("locked"),
            "error should name the directory the walker could not read: {error}"
        );
        assert!(
            error.contains("Permission denied"),
            "error should give the reason the walker reported: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_with_unreadable_directory_and_keep_going_records_the_error() {
        let directory = tree(&["src/main.rs", "locked/inner.rs"]);
        let locked = directory.path().join("locked");
        let _unreadable = make_unreadable(&locked);

        let discovery = Discovery::run(directory.path(), WalkErrorPolicy::ReportAndContinue)
            .expect("discovery should succeed");

        assert_eq!(discovered(&discovery, directory.path()), ["src/main.rs"]);
        assert_eq!(discovery.errors().len(), 1);
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
