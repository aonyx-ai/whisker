use std::fmt;

use gix::url::Scheme;

use crate::config::GitUrl;

/// The host whose repositories the public GitHub API serves
const PUBLIC_HOST: &str = "github.com";

/// A repository named the way GitHub's REST API names one
///
/// A release API answers one question: what does this owner's repository
/// publish. Not every remote a project can pin has that shape. This type
/// says which ones do, and whisker compiles the rest from source.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct GitHubRepository {
    owner: String,
    name: String,
}

impl GitHubRepository {
    /// Returns the repository `url` names, if an API here serves it
    ///
    /// The API always serves a remote on github.com. It also serves one
    /// on `api_host`, which is how a project inside a GitHub Enterprise
    /// installation reaches its own releases. Whoever points whisker at
    /// that API names the host it answers for.
    ///
    /// A remote whisker cannot spell as an owner and a repository is not
    /// one of these. Neither is a local path, because `file://` serves no
    /// releases.
    pub fn from_url(url: &GitUrl, api_host: Option<&str>) -> Option<Self> {
        let url = url.to_gix_url();

        match url.scheme {
            Scheme::Https | Scheme::Http | Scheme::Ssh | Scheme::Git => {}
            Scheme::File | Scheme::Ext | Scheme::Helper(_) | Scheme::HelperUrl(_) => {
                return None;
            }
        }

        let host = url.host()?.to_ascii_lowercase();
        let served = host == PUBLIC_HOST || api_host.is_some_and(|api| host == api);

        if !served {
            return None;
        }

        let path = url.path.to_string();
        let mut segments = path.trim_matches('/').split('/');

        let owner = segments.next()?;
        let name = segments.next()?;

        if segments.next().is_some() {
            return None;
        }

        let name = name.strip_suffix(".git").unwrap_or(name);

        if owner.is_empty() || name.is_empty() {
            return None;
        }

        Some(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }
}

impl fmt::Display for GitHubRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { owner, name } = self;

        write!(f, "{owner}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository(url: &str) -> Option<GitHubRepository> {
        let url = GitUrl::new(url).expect("the remote should be accepted");

        GitHubRepository::from_url(&url, None)
    }

    #[test]
    fn from_url_accepts_an_https_remote() {
        let repository = repository("https://github.com/aonyx-ai/rules");

        assert_eq!(
            repository.map(|it| it.to_string()),
            Some("aonyx-ai/rules".to_owned())
        );
    }

    #[test]
    fn from_url_accepts_the_scp_spelling_of_an_ssh_remote() {
        let repository = repository("git@github.com:aonyx-ai/rules.git");

        assert_eq!(
            repository.map(|it| it.to_string()),
            Some("aonyx-ai/rules".to_owned())
        );
    }

    #[test]
    fn from_url_accepts_an_ssh_remote() {
        let repository = repository("ssh://git@github.com/aonyx-ai/rules.git");

        assert_eq!(
            repository.map(|it| it.to_string()),
            Some("aonyx-ai/rules".to_owned())
        );
    }

    #[test]
    fn from_url_drops_the_git_suffix() {
        let repository = repository("https://github.com/aonyx-ai/rules.git");

        assert_eq!(
            repository.map(|it| it.to_string()),
            Some("aonyx-ai/rules".to_owned())
        );
    }

    #[test]
    fn from_url_accepts_a_trailing_slash() {
        let repository = repository("https://github.com/aonyx-ai/rules/");

        assert_eq!(
            repository.map(|it| it.to_string()),
            Some("aonyx-ai/rules".to_owned())
        );
    }

    /// A host is not case sensitive, and a person pasting one from a
    /// browser can capitalize it.
    #[test]
    fn from_url_accepts_a_host_in_capitals() {
        let repository = repository("https://GitHub.com/aonyx-ai/rules");

        assert_eq!(
            repository.map(|it| it.to_string()),
            Some("aonyx-ai/rules".to_owned())
        );
    }

    /// Credentials belong to whoever fetches, never to the name of the
    /// repository they fetch.
    #[test]
    fn from_url_ignores_credentials() {
        let repository = repository("https://someone:secret@github.com/aonyx-ai/rules");

        assert_eq!(
            repository.map(|it| it.to_string()),
            Some("aonyx-ai/rules".to_owned())
        );
    }

    #[test]
    fn from_url_of_a_configured_api_host_is_served() {
        let url = GitUrl::new("https://github.acme.example/team/rules")
            .expect("the remote should be accepted");

        let repository = GitHubRepository::from_url(&url, Some("github.acme.example"));

        assert_eq!(
            repository.map(|it| it.to_string()),
            Some("team/rules".to_owned())
        );
    }

    #[test]
    fn from_url_of_another_host_is_none() {
        assert_eq!(repository("https://gitlab.com/aonyx-ai/rules"), None);
    }

    #[test]
    fn from_url_of_a_local_path_is_none() {
        assert_eq!(repository("file:///srv/rules"), None);
    }

    #[test]
    fn from_url_of_a_deeper_path_is_none() {
        assert_eq!(
            repository("https://github.com/aonyx-ai/rules/tree/main"),
            None
        );
    }

    #[test]
    fn from_url_of_an_owner_alone_is_none() {
        assert_eq!(repository("https://github.com/aonyx-ai"), None);
    }

    #[test]
    fn from_url_of_an_empty_name_is_none() {
        assert_eq!(repository("https://github.com/aonyx-ai/.git"), None);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GitHubRepository>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GitHubRepository>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<GitHubRepository>();
    }
}
