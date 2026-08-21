/// The remote a git lint source is fetched from
///
/// Whisker passes the text through to the git transport rather than
/// parsing it, so every form the transport accepts keeps working: an
/// `https://` remote, an `ssh://` one, and the `file://` and plain local
/// paths that make a test or a vendored checkout possible. The type earns
/// its place by keeping a remote from being confused with the revision
/// beside it, and by giving the cache one place to derive a directory name.
///
/// # Examples
///
/// ```ignore
/// let url = GitUrl::new("https://github.com/aonyx-ai/whisker-aonyx-rules");
///
/// assert_eq!(url.slug(), "whisker-aonyx-rules");
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct GitUrl(String);

/// The directory name whisker uses when a remote yields no usable segment
const FALLBACK_SLUG: &str = "repository";

impl GitUrl {
    /// Creates a remote from its configured source
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let url = GitUrl::new("https://github.com/aonyx-ai/whisker-aonyx-rules");
    /// ```
    pub fn new(url: impl Into<String>) -> Self {
        Self(url.into())
    }

    /// Returns the remote as written
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let url = GitUrl::new("https://example.com/rules");
    ///
    /// assert_eq!(url.as_str(), "https://example.com/rules");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns a readable directory name for this remote
    ///
    /// The slug exists so that a person looking through the cache can tell
    /// the checkouts apart. It is not an identity: two different remotes
    /// can share a last segment, so the cache pairs the slug with a digest
    /// of the whole remote.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let url = GitUrl::new("https://github.com/aonyx-ai/rules.git");
    ///
    /// assert_eq!(url.slug(), "rules");
    /// ```
    pub fn slug(&self) -> String {
        let slug = self.0.trim_end_matches('/');
        let slug = slug.rsplit(['/', ':']).next().unwrap_or(slug);
        let slug = slug.strip_suffix(".git").unwrap_or(slug);
        let slug: String = slug
            .chars()
            .map(|character| match character.is_ascii_alphanumeric() {
                true => character,
                false => '-',
            })
            .collect();
        let slug = slug.trim_matches('-');

        match slug.is_empty() {
            true => FALLBACK_SLUG.to_owned(),
            false => slug.to_owned(),
        }
    }
}

impl std::fmt::Display for GitUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_the_remote() {
        let url = GitUrl::new("https://example.com/rules");

        assert_eq!(url.as_str(), "https://example.com/rules");
    }

    #[test]
    fn display_matches_source() {
        let url = GitUrl::new("https://example.com/rules");

        assert_eq!(url.to_string(), "https://example.com/rules");
    }

    #[test]
    fn slug_replaces_characters_a_directory_name_should_not_hold() {
        let url = GitUrl::new("https://example.com/we ird");

        assert_eq!(url.slug(), "we-ird");
    }

    #[test]
    fn slug_of_a_local_path_uses_the_last_segment() {
        let url = GitUrl::new("/src/checkouts/rules");

        assert_eq!(url.slug(), "rules");
    }

    #[test]
    fn slug_of_an_scp_style_remote_uses_the_last_segment() {
        let url = GitUrl::new("git@github.com:aonyx-ai/rules.git");

        assert_eq!(url.slug(), "rules");
    }

    #[test]
    fn slug_strips_a_git_suffix() {
        let url = GitUrl::new("https://github.com/aonyx-ai/rules.git");

        assert_eq!(url.slug(), "rules");
    }

    #[test]
    fn slug_strips_a_trailing_separator() {
        let url = GitUrl::new("https://github.com/aonyx-ai/rules/");

        assert_eq!(url.slug(), "rules");
    }

    #[test]
    fn slug_with_no_usable_segment_falls_back() {
        let url = GitUrl::new("///");

        assert_eq!(url.slug(), FALLBACK_SLUG);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GitUrl>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GitUrl>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<GitUrl>();
    }
}
