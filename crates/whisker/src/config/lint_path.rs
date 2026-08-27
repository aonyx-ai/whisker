use std::path::PathBuf;

use kawauso_project::project::ProjectRoot;

/// The path of a custom lint crate configured in the whisker configuration
///
/// The path names a directory holding a Cargo package that exports lint
/// passes with `export_lints!`. The constructor does not validate it; the
/// plugin loader resolves and checks the path when the run starts, so the
/// error can say which configured entry is broken.
///
/// # Examples
///
/// ```ignore
/// let path = LintPath::new("lints/no_todo");
///
/// assert_eq!(path.resolve(&root), Path::new("/project/lints/no_todo"));
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct LintPath(PathBuf);

impl LintPath {
    /// Creates a lint path from its configured source
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let path = LintPath::new("lints/no_todo");
    /// ```
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Returns the path this entry points at, anchored at `root`
    ///
    /// A relative path anchors at the project directory, the same directory
    /// the ignore patterns anchor at. An absolute path stands on its own.
    ///
    /// The anchor is a [`ProjectRoot`] rather than any path, because the
    /// only directory a configured entry may anchor at is the one the search
    /// found.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let path = LintPath::new("lints/no_todo");
    ///
    /// assert_eq!(path.resolve(&root), Path::new("/project/lints/no_todo"));
    /// ```
    pub fn resolve(&self, root: &ProjectRoot) -> PathBuf {
        root.get().join(&self.0)
    }
}

impl std::fmt::Display for LintPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.display().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Returns a project root for an entry under test to anchor at
    fn root() -> ProjectRoot {
        ProjectRoot::new(PathBuf::from("/project"))
    }

    #[test]
    fn display_matches_source() {
        let path = LintPath::new("lints/no_todo");

        assert_eq!(path.to_string(), "lints/no_todo");
    }

    #[test]
    fn resolve_anchors_a_relative_path_at_the_root() {
        let path = LintPath::new("lints/no_todo");

        let resolved = path.resolve(&root());

        assert_eq!(resolved, Path::new("/project/lints/no_todo"));
    }

    #[test]
    fn resolve_keeps_an_absolute_path() {
        let path = LintPath::new("/elsewhere/no_todo");

        let resolved = path.resolve(&root());

        assert_eq!(resolved, Path::new("/elsewhere/no_todo"));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LintPath>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LintPath>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LintPath>();
    }
}
