/// A gitignore-syntax pattern that excludes files from a `whisker check` run
///
/// The syntax is gitignore's, so projects do not need a second pattern
/// language. A trailing slash restricts a pattern to directories, `**`
/// crosses directory boundaries, and a leading `!` re-includes an earlier
/// exclusion. As in gitignore, `!` cannot re-include a file whose parent
/// directory is excluded, because the walk never enters that directory.
///
/// Patterns match against the project directory that holds the
/// configuration file, not against the run's starting directory. A bare
/// `examples/` matches at any depth beneath that directory, and
/// `/examples/` matches only at the top of it.
///
/// # Examples
///
/// ```ignore
/// let pattern = IgnorePattern::new("examples/");
///
/// assert_eq!(pattern.as_str(), "examples/");
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct IgnorePattern(String);

impl IgnorePattern {
    /// Creates a pattern from its gitignore-syntax source
    ///
    /// The constructor does not validate the source. Discovery compiles the
    /// patterns later and rejects invalid syntax with an error that names
    /// the pattern.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pattern = IgnorePattern::new("target/");
    /// ```
    pub fn new(pattern: impl Into<String>) -> Self {
        Self(pattern.into())
    }

    /// Returns the gitignore-syntax source of this pattern
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(IgnorePattern::new("target/").as_str(), "target/");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IgnorePattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_source() {
        let pattern = IgnorePattern::new("examples/");

        assert_eq!(pattern.as_str(), "examples/");
    }

    #[test]
    fn display_matches_source() {
        let pattern = IgnorePattern::new("crates/*/tests/fixtures/");

        assert_eq!(pattern.to_string(), "crates/*/tests/fixtures/");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<IgnorePattern>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<IgnorePattern>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<IgnorePattern>();
    }
}
