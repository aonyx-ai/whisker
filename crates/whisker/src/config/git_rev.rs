/// The full commit hash a git lint source is pinned to
///
/// Whisker accepts only a complete, unabbreviated hash. A branch or tag
/// names whatever the remote points it at today, so the same configuration
/// would check the same code against different rules on different days,
/// and a shortened hash can grow ambiguous as a repository gains objects.
/// Neither is something a linter should leave open, so the pin is exact.
///
/// # Examples
///
/// ```ignore
/// let rev = GitRev::new("0123456789abcdef0123456789abcdef01234567")?;
///
/// assert_eq!(rev.as_str().len(), 40);
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct GitRev(String);

/// The number of hexadecimal characters in a full SHA-1 commit hash
const HASH_LENGTH: usize = 40;

impl GitRev {
    /// Creates a revision from its configured source
    ///
    /// # Errors
    ///
    /// Returns an error if `rev` is not exactly 40 lowercase hexadecimal
    /// characters. The error explains which rule the value broke, because
    /// the most likely mistakes are a branch name and an abbreviated hash,
    /// and each of those is a different fix.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let rev = GitRev::new("0123456789abcdef0123456789abcdef01234567")?;
    /// ```
    pub fn new(rev: impl Into<String>) -> anyhow::Result<Self> {
        let rev = rev.into();

        anyhow::ensure!(
            rev.len() == HASH_LENGTH,
            "the lint source revision {rev:?} is not a full commit hash; whisker pins lints to \
             all {HASH_LENGTH} characters of one commit so that a check runs the same rules every \
             time, which a branch name or an abbreviated hash cannot promise"
        );
        anyhow::ensure!(
            rev.chars()
                .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character)),
            "the lint source revision {rev:?} is not a commit hash; a hash is written in \
             lowercase hexadecimal"
        );

        Ok(Self(rev))
    }

    /// Returns the hash as written
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let rev = GitRev::new("0123456789abcdef0123456789abcdef01234567")?;
    ///
    /// assert!(rev.as_str().starts_with("0123"));
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GitRev {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn as_str_returns_the_hash() {
        let rev = GitRev::new(VALID).expect("the revision should be accepted");

        assert_eq!(rev.as_str(), VALID);
    }

    #[test]
    fn display_matches_source() {
        let rev = GitRev::new(VALID).expect("the revision should be accepted");

        assert_eq!(rev.to_string(), VALID);
    }

    #[test]
    fn new_with_abbreviated_hash_returns_error() {
        let error = GitRev::new("0123456").expect_err("the revision should be rejected");

        assert!(
            error.to_string().contains("full commit hash"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn new_with_branch_name_returns_error() {
        let error = GitRev::new("main").expect_err("the revision should be rejected");

        assert!(
            error.to_string().contains("full commit hash"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn new_with_full_hash_is_accepted() {
        let rev = GitRev::new(VALID).expect("the revision should be accepted");

        assert_eq!(rev.as_str(), VALID);
    }

    #[test]
    fn new_with_non_hexadecimal_characters_returns_error() {
        let rev = "z".repeat(HASH_LENGTH);

        let error = GitRev::new(rev).expect_err("the revision should be rejected");

        assert!(
            error.to_string().contains("lowercase hexadecimal"),
            "unexpected: {error:#}"
        );
    }

    /// An uppercase hash is a hash, but accepting it would let one commit
    /// be written two ways, and the cache is keyed by the text.
    #[test]
    fn new_with_uppercase_hash_returns_error() {
        let error = GitRev::new(VALID.to_uppercase()).expect_err("the revision should be rejected");

        assert!(
            error.to_string().contains("lowercase hexadecimal"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GitRev>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GitRev>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<GitRev>();
    }
}
