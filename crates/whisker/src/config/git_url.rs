/// The remote a git lint source is fetched from
///
/// Git accepts more forms than a general URL parser does. An `https://` or
/// an `ssh://` remote, an scp-like `git@host:org/repo`, and the `file://`
/// and plain local paths a test or a vendored checkout uses all name a
/// repository.
///
/// Whisker therefore reads a remote with gitoxide's parser, which the fetch
/// uses later too. A remote the configuration accepts is a remote the
/// transport accepts. One that nothing can read fails while the file is
/// read, rather than minutes into a check.
///
/// The parsed form also makes the credential rule structural: a token is a
/// field to clear rather than a substring to find.
///
/// # Examples
///
/// ```ignore
/// let url = GitUrl::new("https://github.com/aonyx-ai/whisker-aonyx-rules")?;
///
/// assert_eq!(
///     url.to_string(),
///     "https://github.com/aonyx-ai/whisker-aonyx-rules"
/// );
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GitUrl(gix::url::Url);

/// What stands in for credentials when a remote is printed
const REDACTED_USERINFO: &str = "***";

/// The directory name whisker uses when a remote yields no usable segment
const FALLBACK_SLUG: &str = "repository";

impl GitUrl {
    /// Creates a remote from its configured source
    ///
    /// # Errors
    ///
    /// Returns an error if git cannot read the text as a remote. The error
    /// names the fault and never quotes the remote, because gitoxide's own
    /// parse error carries the text in full, credentials and all.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let url = GitUrl::new("https://github.com/aonyx-ai/whisker-aonyx-rules")?;
    /// ```
    pub fn new(url: impl AsRef<str>) -> anyhow::Result<Self> {
        match gix::url::parse(url.as_ref()) {
            Ok(url) => Ok(Self(url)),
            Err(gix::url::parse::Error::Utf8 { .. }) => {
                anyhow::bail!("the git remote is not valid UTF-8")
            }
            Err(gix::url::parse::Error::Url { .. }) => {
                anyhow::bail!("cannot read the git remote as a URL")
            }
            Err(gix::url::parse::Error::TooLong { .. }) => {
                anyhow::bail!("the git remote names a host that is too long")
            }
            Err(gix::url::parse::Error::MissingRepositoryPath { .. }) => {
                anyhow::bail!("the git remote names no repository")
            }
            Err(gix::url::parse::Error::RelativeUrl { .. }) => {
                anyhow::bail!("the git remote is relative")
            }
            Err(gix::url::parse::Error::InvalidRemoteHelperName { .. }) => {
                anyhow::bail!("the git remote names an invalid transport")
            }
        }
    }

    /// Returns the remote in the form the transport reads
    ///
    /// The fetch needs the remote as configured, credentials and all, which
    /// is what separates this from [`Display`]. Gitoxide takes the parsed
    /// form directly, so the remote whisker accepted is the remote it
    /// connects to. No second parse can disagree with the first.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let remote = repository.remote_at(url.to_gix_url())?;
    /// ```
    ///
    /// [`Display`]: std::fmt::Display
    pub fn to_gix_url(&self) -> gix::url::Url {
        self.0.clone()
    }

    /// Returns a readable directory name for this remote
    ///
    /// The slug exists so that a person looking through the cache can tell
    /// the checkouts apart. It is not an identity: two different remotes
    /// can share a last path segment. The cache therefore pairs the slug
    /// with a digest of the whole remote.
    ///
    /// The name comes from the parsed path, so every form of remote is read
    /// the same way. An scp-like `git@host:org/repo` and an `https://` URL
    /// spell their separators differently, and only the parser knows which
    /// part of either is the path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let url = GitUrl::new("https://github.com/aonyx-ai/rules.git")?;
    ///
    /// assert_eq!(url.slug(), "rules");
    /// ```
    pub fn slug(&self) -> String {
        let path = String::from_utf8_lossy(&self.0.path);
        let slug = path.trim_end_matches('/');
        let slug = slug.rsplit('/').next().unwrap_or(slug);
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

/// Returns the remote with any credentials removed
///
/// A remote may carry a token, either as `https://<token>@host` or as
/// `https://user:<token>@host`, and whisker prints the remote whenever a
/// lint source fails. Stderr becomes a CI log, so the whole userinfo goes
/// rather than the password half of it. Both forms carry secrets, and one
/// rule cannot be applied to the wrong one.
///
/// [`gix::url::Url`] hides only the password and prints the name beside it,
/// which is why this does not delegate to its own [`Display`].
///
/// An scp-style remote such as `git@github.com:org/repo` keeps its name.
/// A name there is an ssh login rather than a place a token goes, and the
/// form has no other way to spell one.
///
/// [`Display`]: std::fmt::Display
fn redacted(url: &gix::url::Url) -> String {
    let mut redacted = url.clone();

    redacted.set_password(None);
    redacted.set_user(match (url.serialize_alternative_form, url.user()) {
        (true, user) => user.map(str::to_owned),
        (false, Some(_)) => Some(REDACTED_USERINFO.to_owned()),
        (false, None) => None,
    });

    redacted.to_bstring().to_string()
}

impl std::fmt::Display for GitUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        redacted(&self.0).fmt(f)
    }
}

/// Prints the remote the way [`Display`] does, credentials removed
///
/// The derive would print the field as written, and a `{:?}` of a
/// configuration reaches a panic message as easily as a `{}` reaches
/// stderr, so both go through the same redaction.
///
/// [`Display`]: std::fmt::Display
impl std::fmt::Debug for GitUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GitUrl").field(&redacted(&self.0)).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_credentials() {
        let url = GitUrl::new("https://x-access-token:secret@example.com/rules")
            .expect("the remote should be accepted");

        let shown = format!("{url:?}");

        assert!(!shown.contains("secret"), "{shown}");
    }

    #[test]
    fn display_hides_a_password() {
        let url = GitUrl::new("https://user:secret@example.com/rules")
            .expect("the remote should be accepted");

        assert_eq!(url.to_string(), "https://***@example.com/rules");
    }

    #[test]
    fn display_hides_a_password_that_holds_a_marker() {
        let url = GitUrl::new("https://user:pass@word@example.com/rules")
            .expect("the remote should be accepted");

        assert_eq!(url.to_string(), "https://***@example.com/rules");
    }

    #[test]
    fn display_hides_a_token_used_as_a_name() {
        let url =
            GitUrl::new("https://secret@example.com/rules").expect("the remote should be accepted");

        assert_eq!(url.to_string(), "https://***@example.com/rules");
    }

    /// Pins that a remote whisker never has to fetch over is left alone
    ///
    /// A local path is how a test and a vendored checkout name their rules,
    /// and it carries no credentials to remove.
    #[test]
    fn display_keeps_a_local_path_whole() {
        let url = GitUrl::new("/srv/rules").expect("the remote should be accepted");

        assert_eq!(url.to_string(), "/srv/rules");
    }

    #[test]
    fn display_keeps_an_scp_style_remote_whole() {
        let url = GitUrl::new("git@github.com:aonyx-ai/rules.git")
            .expect("the remote should be accepted");

        assert_eq!(url.to_string(), "git@github.com:aonyx-ai/rules.git");
    }

    #[test]
    fn display_matches_a_remote_without_credentials() {
        let url = GitUrl::new("https://example.com/rules").expect("the remote should be accepted");

        assert_eq!(url.to_string(), "https://example.com/rules");
    }

    /// Pins that credentials go even where there is no path to keep
    ///
    /// The redaction rebuilds the remote from its parts rather than cutting
    /// the text apart. A remote with nothing after the host is where a rule
    /// that reads from the left would run past the end.
    #[test]
    fn display_of_a_remote_without_a_path_hides_credentials() {
        let url = GitUrl::new("https://secret@example.com").expect("the remote should be accepted");

        assert_eq!(url.to_string(), "https://***@example.com/");
    }

    #[test]
    fn new_with_an_empty_remote_returns_error() {
        let error = GitUrl::new("").expect_err("the remote should be rejected");

        assert!(
            error.to_string().contains("names no repository"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn slug_of_a_local_path_uses_the_last_segment() {
        let url = GitUrl::new("/src/checkouts/rules").expect("the remote should be accepted");

        assert_eq!(url.slug(), "rules");
    }

    #[test]
    fn slug_of_an_scp_style_remote_uses_the_last_segment() {
        let url = GitUrl::new("git@github.com:aonyx-ai/rules.git")
            .expect("the remote should be accepted");

        assert_eq!(url.slug(), "rules");
    }

    #[test]
    fn slug_replaces_characters_a_directory_name_should_not_hold() {
        let url = GitUrl::new("https://example.com/we_ird").expect("the remote should be accepted");

        assert_eq!(url.slug(), "we-ird");
    }

    #[test]
    fn slug_strips_a_git_suffix() {
        let url = GitUrl::new("https://github.com/aonyx-ai/rules.git")
            .expect("the remote should be accepted");

        assert_eq!(url.slug(), "rules");
    }

    #[test]
    fn slug_strips_a_trailing_separator() {
        let url = GitUrl::new("https://github.com/aonyx-ai/rules/")
            .expect("the remote should be accepted");

        assert_eq!(url.slug(), "rules");
    }

    #[test]
    fn slug_with_no_usable_segment_falls_back() {
        let url = GitUrl::new("https://example.com/").expect("the remote should be accepted");

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
