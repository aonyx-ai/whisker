/// A pattern whose matches are kept out of a `whisker check` run
///
/// The syntax is gitignore's rather than a bespoke glob dialect. Projects
/// already write and read gitignore patterns, whisker already honors the
/// `.gitignore` files it finds, and inventing a second pattern language would
/// mean two sets of rules to reason about when a file unexpectedly is or is
/// not linted. A trailing slash therefore restricts a pattern to directories,
/// `**` crosses directory boundaries, and a leading `!` re-includes something
/// an earlier pattern excluded. Inheriting gitignore syntax also means
/// inheriting its one surprise: a `!` cannot re-include a file whose parent
/// directory is itself excluded, because the walk never descends into that
/// directory to reach the file.
///
/// A pattern is matched against the Cargo workspace root rather than against
/// the directory whisker was pointed at, so it means the same thing whether
/// the run starts at the workspace root or inside a member crate. Within that
/// root it anchors the way gitignore anchors: `crates/app/generated/` names
/// one directory relative to the root, a bare `examples/` matches a directory
/// of that name at any depth beneath it, and `/examples/` restricts it to the
/// root.
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
    /// The source is not validated here. A pattern is only compiled once the
    /// workspace root it is anchored to is known, which is where a syntax
    /// error can be reported against the manifest that introduced it.
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
