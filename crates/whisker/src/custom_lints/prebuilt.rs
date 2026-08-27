//! Lint libraries that someone else already built
//!
//! A git lint source is a repository of cargo packages, and whisker turns
//! it into loadable libraries by compiling it. That costs a toolchain and
//! several minutes, and it costs them again in every project and on every
//! build agent that checks out the same pin.
//!
//! It is also the one step whisker cannot take on a machine that has no
//! matching toolchain, which a machine running a released whisker binary
//! usually has not: the handshake accepts a library only from the rustc
//! that built whisker itself.
//!
//! So whisker looks for libraries that were built once, by whoever
//! publishes the lints, against the whisker that is asking. The
//! [`AbiTag`] is how it asks. This module owns what happens with the
//! answer: where it is kept, and which files in it are libraries.
//!
//! Nothing here is trusted for being prebuilt. Every library still goes
//! through the same handshake as one whisker compiled itself, and a
//! library that fails it is an error rather than a reason to fall back:
//! the tag is derived from exactly what the handshake compares, so a
//! mismatch means the archive was misnamed, and building from source
//! instead would hide that.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

use super::abi_tag::AbiTag;
use super::cache;
use crate::config::GitLintSource;

/// Returns the directory holding prebuilt lints for `source`, if any
///
/// Whisker keeps what it unpacks, so a run that finds its directory
/// touches nothing else. An empty directory does not count as a find.
/// An interrupted unpack leaves one behind, and whisker replaces it.
///
/// # Errors
///
/// Returns an error if no cache location can be determined, which is the
/// same condition that stops a git checkout.
pub fn resolve(source: &GitLintSource, tag: &AbiTag) -> anyhow::Result<Option<PathBuf>> {
    let directory = cache::prebuilt_directory(source, tag)?;

    match holds_libraries(&directory) {
        true => Ok(Some(directory)),
        false => Ok(None),
    }
}

/// Returns every dynamic library directly inside `directory`, in order
///
/// A publisher ships one library per lint package, so a directory holds
/// as many as the repository built. Whisker ignores every other file, so
/// a publisher may add a manifest or a license beside them.
///
/// Whisker sorts by file name, so two runs load the same lints in the
/// same order and report the same diagnostics.
///
/// # Errors
///
/// Returns an error if the directory cannot be read.
pub fn libraries(directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?;

    let mut libraries = Vec::new();

    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read an entry of {}", directory.display()))?;
        let path = entry.path();

        if path.extension() != Some(OsStr::new(std::env::consts::DLL_EXTENSION)) {
            continue;
        }

        if !path.is_file() {
            continue;
        }

        libraries.push(path);
    }

    libraries.sort();

    Ok(libraries)
}

/// Returns whether `directory` holds at least one loadable library
///
/// A directory whisker cannot read counts as empty. Whisker replaces
/// such a cache entry, and a run continues.
fn holds_libraries(directory: &Path) -> bool {
    libraries(directory).is_ok_and(|libraries| !libraries.is_empty())
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tempfile::TempDir;

    use super::*;

    /// Writes `names` as empty files in a new directory
    fn directory_holding(names: &[&str]) -> TempDir {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        for name in names {
            File::create(directory.path().join(name)).expect("the file should be created");
        }

        directory
    }

    /// Returns a file name that whisker would load on this platform
    fn library(stem: &str) -> String {
        format!(
            "{}{stem}.{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_EXTENSION
        )
    }

    #[test]
    fn holds_libraries_with_a_library_is_true() {
        let directory = directory_holding(&[&library("one")]);

        assert!(holds_libraries(directory.path()));
    }

    #[test]
    fn holds_libraries_with_nothing_to_load_is_false() {
        let directory = directory_holding(&["README.md"]);

        assert!(!holds_libraries(directory.path()));
    }

    /// An unpack that was interrupted leaves a directory behind, and the
    /// next run has to be able to replace it rather than fail on it.
    #[test]
    fn holds_libraries_of_a_missing_directory_is_false() {
        let directory = directory_holding(&[]);
        let missing = directory.path().join("absent");

        assert!(!holds_libraries(&missing));
    }

    #[test]
    fn libraries_are_ordered_by_name() {
        let directory = directory_holding(&[&library("b"), &library("a"), &library("c")]);

        let libraries = libraries(directory.path()).expect("the directory should be read");

        let names: Vec<_> = libraries
            .iter()
            .filter_map(|path| path.file_name())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();

        assert_eq!(names, sorted);
    }

    #[test]
    fn libraries_ignores_a_directory_that_looks_like_one() {
        let directory = directory_holding(&[&library("real")]);
        std::fs::create_dir(directory.path().join(library("fake")))
            .expect("the directory should be created");

        let libraries = libraries(directory.path()).expect("the directory should be read");

        assert_eq!(libraries.len(), 1, "{libraries:?}");
    }

    #[test]
    fn libraries_ignores_everything_that_is_not_a_library() {
        let directory = directory_holding(&[&library("one"), "README.md", "notes.txt", "plain"]);

        let libraries = libraries(directory.path()).expect("the directory should be read");

        assert_eq!(libraries.len(), 1, "{libraries:?}");
    }

    #[test]
    fn libraries_of_an_empty_directory_is_empty() {
        let directory = directory_holding(&[]);

        let libraries = libraries(directory.path()).expect("the directory should be read");

        assert!(libraries.is_empty(), "{libraries:?}");
    }

    #[test]
    fn libraries_of_a_missing_directory_returns_error() {
        let directory = directory_holding(&[]);
        let missing = directory.path().join("absent");

        let error = libraries(&missing).expect_err("should fail");

        assert!(format!("{error:#}").contains("absent"), "{error:#}");
    }
}
