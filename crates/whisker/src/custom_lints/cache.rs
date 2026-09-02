//! Where whisker keeps what it fetches for custom lints
//!
//! A lint source that is not already a directory on the machine has to
//! become one before whisker can use it. This module owns the layout of
//! the cache those arrive in: where its root is, where one source sits
//! under it, and where a fetch assembles a source before it is whole
//! enough to install.
//!
//! The layout is written into directory names that outlive the process,
//! so each part of it is stable by construction rather than by habit.
//! The module sits beside the sources rather than inside one of them,
//! because the cache is the platform's, not any one source's.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use etcetera::base_strategy::{BaseStrategy as _, Xdg};

use super::digest::digest;
use crate::config::GitLintSource;

/// The environment variable that moves the cache somewhere else
///
/// Tests set it so that a run never reads or writes the cache of the
/// person running them, and a continuous integration job sets it to a
/// directory it knows how to restore between runs.
const CACHE_VARIABLE: &str = "WHISKER_CACHE_DIR";

/// Where whisker may keep the lint sources it fetches
///
/// The candidates are read from the environment in one place and resolved
/// in another, so the precedence between them can be tested without a
/// process to set variables in.
///
/// Only the override is whisker's own. The directory beside it comes from
/// etcetera, which reads `XDG_CACHE_HOME` and falls back to the home
/// directory the way the base directory specification asks for, including
/// ignoring a value that is not an absolute path.
#[derive(Clone, Eq, PartialEq, Debug)]
struct CacheLocation {
    override_directory: Option<OsString>,
    base_directory: Option<PathBuf>,
}

impl CacheLocation {
    /// Reads the candidate locations from the environment
    ///
    /// An empty override counts as unset, because that is what a shell
    /// leaves behind when it expands something that was never set, and
    /// joining onto it would put the cache at the filesystem root.
    fn from_environment() -> Self {
        Self {
            override_directory: present(std::env::var_os(CACHE_VARIABLE)),
            base_directory: Xdg::new().ok().map(|base| base.cache_dir()),
        }
    }

    /// Returns the directory whisker should keep fetched sources in
    ///
    /// # Errors
    ///
    /// Returns an error if neither candidate is set, and names the override
    /// in it, because that is the one the reader can act on.
    fn resolve(&self) -> anyhow::Result<PathBuf> {
        let Self {
            override_directory,
            base_directory,
        } = self;

        if let Some(directory) = override_directory {
            return Ok(PathBuf::from(directory));
        }

        let base_directory = base_directory.as_ref().with_context(|| {
            format!(
                "failed to find a cache directory for git lint sources; set {CACHE_VARIABLE} to \
                 the directory whisker should keep them in"
            )
        })?;

        Ok(base_directory.join("whisker"))
    }
}

/// Returns the directory whisker keeps fetched lint sources in
///
/// # Errors
///
/// Returns an error if the `WHISKER_CACHE_DIR` override is unset or empty
/// and no home directory can be found.
///
/// # Examples
///
/// ```ignore
/// let root = cache_root()?;
///
/// println!("checkouts live under {}", root.display());
/// ```
pub fn cache_root() -> anyhow::Result<PathBuf> {
    CacheLocation::from_environment().resolve()
}

/// Returns the directory one pinned lint source is checked out in
///
/// # Errors
///
/// Returns an error if no cache location can be determined.
pub fn checkout_directory(source: &GitLintSource) -> anyhow::Result<PathBuf> {
    let root = cache_root()?;

    Ok(checkout_directory_in(&root, source))
}

/// Returns where one pinned lint source sits under `root`
///
/// The remote contributes a readable name so that the cache can be
/// browsed, and a digest so that two remotes ending in the same segment
/// stay apart. The commit is the last segment rather than part of the
/// name, which keeps every checkout of one remote together.
///
/// The digest reads the remote as it prints, credentials removed, so two
/// people holding different tokens for one repository share a checkout
/// rather than fetching the same commit twice.
fn checkout_directory_in(root: &Path, source: &GitLintSource) -> PathBuf {
    let remote = format!(
        "{}-{}",
        source.url().slug(),
        digest(&source.url().to_string())
    );

    root.join("git").join(remote).join(source.rev().as_str())
}

/// Returns the directory a checkout is assembled in before it is installed
///
/// A fetch writes here and is renamed into place only once it is whole, so
/// a run interrupted halfway leaves nothing another run could mistake for
/// a finished checkout. The process id keeps two whiskers on one machine
/// from writing into the same half-built tree.
pub fn staging_directory(destination: &Path) -> PathBuf {
    let mut name = destination.as_os_str().to_os_string();
    name.push(format!(".{}.partial", std::process::id()));

    PathBuf::from(name)
}

/// Returns a variable's value, treating an empty one as unset
///
/// An empty variable is what a shell leaves behind when it expands
/// something that was never set. Joining onto it would put the cache at
/// the filesystem root, so whisker reads it as the absence it is.
fn present(value: Option<OsString>) -> Option<OsString> {
    value.filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GitRev, GitUrl};

    const REV: &str = "0123456789abcdef0123456789abcdef01234567";
    const ROOT: &str = "/cache";

    fn location(override_directory: Option<&str>, base_directory: Option<&str>) -> CacheLocation {
        CacheLocation {
            override_directory: override_directory.map(OsString::from),
            base_directory: base_directory.map(PathBuf::from),
        }
    }

    fn source(url: &str) -> GitLintSource {
        GitLintSource::new(
            GitUrl::new(url).expect("the remote should be accepted"),
            GitRev::new(REV).expect("the revision should be accepted"),
        )
    }

    #[test]
    fn checkout_directory_in_ends_with_the_commit() {
        let directory =
            checkout_directory_in(Path::new(ROOT), &source("https://example.com/rules"));

        assert_eq!(
            directory.file_name().expect("should have a name"),
            std::ffi::OsStr::new(REV)
        );
    }

    /// Pins that a token does not give a repository a cache of its own
    ///
    /// Two people can hold different tokens for one private repository, and
    /// the commit either of them names is the same tree. The digest reads
    /// the printed remote, which has no credentials in it.
    #[test]
    fn checkout_directory_in_ignores_credentials() {
        let first = checkout_directory_in(Path::new(ROOT), &source("https://a@example.com/rules"));
        let second = checkout_directory_in(Path::new(ROOT), &source("https://b@example.com/rules"));

        assert_eq!(first, second);
    }

    #[test]
    fn checkout_directory_in_keeps_one_remote_in_one_place() {
        let first = checkout_directory_in(
            Path::new(ROOT),
            &GitLintSource::new(
                GitUrl::new("https://example.com/rules").expect("the remote should be accepted"),
                GitRev::new("a".repeat(40)).expect("the revision should be accepted"),
            ),
        );
        let second = checkout_directory_in(
            Path::new(ROOT),
            &GitLintSource::new(
                GitUrl::new("https://example.com/rules").expect("the remote should be accepted"),
                GitRev::new("b".repeat(40)).expect("the revision should be accepted"),
            ),
        );

        assert_eq!(first.parent(), second.parent());
        assert_ne!(first, second);
    }

    #[test]
    fn checkout_directory_in_names_the_remote_readably() {
        let directory = checkout_directory_in(
            Path::new(ROOT),
            &source("https://github.com/aonyx-ai/rules.git"),
        );

        let remote = directory
            .parent()
            .and_then(Path::file_name)
            .expect("should have a remote directory")
            .to_string_lossy()
            .into_owned();

        assert!(remote.starts_with("rules-"), "unexpected: {remote}");
    }

    /// Two remotes can end in the same segment, and a checkout of one must
    /// never be served for the other.
    #[test]
    fn checkout_directory_in_separates_remotes_that_share_a_last_segment() {
        let first = checkout_directory_in(Path::new(ROOT), &source("https://example.com/a/rules"));
        let second = checkout_directory_in(Path::new(ROOT), &source("https://example.com/b/rules"));

        assert_ne!(first, second);
    }

    #[test]
    fn resolve_names_whisker_under_the_base_directory() {
        let location = location(None, Some("/xdg"));

        let root = location.resolve().expect("should resolve");

        assert_eq!(root, Path::new("/xdg/whisker"));
    }

    #[test]
    fn resolve_prefers_the_override() {
        let location = location(Some(ROOT), Some("/xdg"));

        let root = location.resolve().expect("should resolve");

        assert_eq!(root, Path::new(ROOT));
    }

    #[test]
    fn resolve_without_any_location_returns_error() {
        let location = location(None, None);

        let error = location.resolve().expect_err("should fail");

        assert!(
            format!("{error:#}").contains(CACHE_VARIABLE),
            "the error should name the way out: {error:#}"
        );
    }

    /// A shell that expands an unset variable leaves an empty string
    /// behind, and joining onto that would put the cache at the root.
    #[test]
    fn present_treats_an_empty_value_as_unset() {
        let value = present(Some(OsString::new()));

        assert_eq!(value, None);
    }

    #[test]
    fn present_keeps_a_value_that_holds_something() {
        let value = present(Some(OsString::from("/cache")));

        assert_eq!(value, Some(OsString::from("/cache")));
    }

    #[test]
    fn staging_directory_is_a_sibling_of_the_destination() {
        let staging = staging_directory(Path::new("/cache/git/rules-0/abc"));

        assert_eq!(staging.parent(), Some(Path::new("/cache/git/rules-0")));
        assert_ne!(staging, PathBuf::from("/cache/git/rules-0/abc"));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CacheLocation>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CacheLocation>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CacheLocation>();
    }
}
