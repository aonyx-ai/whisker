use crate::config::{GitRev, GitUrl, LintPath};

/// Where one configured set of custom lints comes from
///
/// A project either keeps its lints beside its code or shares them with
/// other projects through a repository. The two arrive at the same place,
/// a directory holding cargo packages that whisker builds and loads, so
/// they differ only in how that directory comes to exist: a path is
/// already there, and a git source is fetched into the cache first.
///
/// # Examples
///
/// ```ignore
/// let source = LintSource::Path(LintPath::new("lints/no_todo"));
///
/// assert_eq!(source.to_string(), "lints/no_todo");
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum LintSource {
    /// Lints in a directory of the project or of the machine
    Path(LintPath),

    /// Lints in a repository, pinned to one commit
    Git(GitLintSource),
}

impl std::fmt::Display for LintSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(path) => path.fmt(f),
            Self::Git(git) => git.fmt(f),
        }
    }
}

/// A repository and the commit whisker checks its lints out at
///
/// Both halves are required, and the [`GitRev`] is a full hash, so the
/// source names exactly one tree. That is what lets whisker treat a
/// checkout as permanent once it exists: the cache never has to ask
/// whether the remote moved, because this source cannot follow it.
///
/// # Examples
///
/// ```ignore
/// let source = GitLintSource::new(
///     GitUrl::new("https://github.com/aonyx-ai/whisker-aonyx-rules")?,
///     GitRev::new("0123456789abcdef0123456789abcdef01234567")?,
/// );
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct GitLintSource {
    url: GitUrl,
    rev: GitRev,
}

impl GitLintSource {
    /// Creates a git lint source from a remote and a pinned commit
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let source = GitLintSource::new(url, rev);
    /// ```
    pub fn new(url: GitUrl, rev: GitRev) -> Self {
        Self { url, rev }
    }
}

impl std::fmt::Display for GitLintSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}", self.url, self.rev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REV: &str = "0123456789abcdef0123456789abcdef01234567";

    fn git_source() -> GitLintSource {
        GitLintSource::new(
            GitUrl::new("https://example.com/rules").expect("the remote should be accepted"),
            GitRev::new(REV).expect("the revision should be accepted"),
        )
    }

    #[test]
    fn display_of_a_git_source_names_the_remote_and_the_commit() {
        let source = LintSource::Git(git_source());

        let displayed = source.to_string();

        assert!(
            displayed.contains("https://example.com/rules"),
            "{displayed}"
        );
        assert!(displayed.contains(REV), "{displayed}");
    }

    #[test]
    fn display_of_a_path_source_matches_the_path() {
        let source = LintSource::Path(LintPath::new("lints/no_todo"));

        assert_eq!(source.to_string(), "lints/no_todo");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LintSource>();
        assert_send::<GitLintSource>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LintSource>();
        assert_sync::<GitLintSource>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LintSource>();
        assert_unpin::<GitLintSource>();
    }
}
