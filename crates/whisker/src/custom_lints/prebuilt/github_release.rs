use std::io::Read;
use std::path::Path;
use std::time::Duration;

use anyhow::Context as _;
use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;

use super::archive::Sha256Digest;
use super::asset_name::AssetName;
use super::github_repository::GitHubRepository;

/// The API whisker asks when nothing else is configured
const DEFAULT_BASE: &str = "https://api.github.com";

/// The variable that points whisker at another release API
///
/// A GitHub Enterprise installation serves the same API under its own
/// name. The tests serve it from a local port.
const API_VARIABLE: &str = "WHISKER_GITHUB_API_URL";

/// The variables a token is read from, in the order `gh` reads them
const TOKEN_VARIABLES: [&str; 2] = ["GH_TOKEN", "GITHUB_TOKEN"];

/// How long whisker waits to reach the API before giving up
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The most of a release listing whisker reads
///
/// A hundred releases with three files each sit far below this. The limit
/// exists so that an endless answer costs a failed check and not the
/// machine's memory.
const MAX_LISTING: u64 = 8 * 1024 * 1024;

/// The most of a digest file whisker reads
///
/// A sidecar holds one line.
const MAX_SIDECAR: u64 = 4 * 1024;

/// The most of an archive whisker writes to disk
///
/// A repository of rules builds one library per rule, so a real archive
/// holds tens of megabytes. This limit sits far above that, and far below
/// a full disk.
const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;

/// How long whisker waits for one request to finish
///
/// One limit covers a small listing and a large archive. It allows a
/// download on a slow link, and it still releases a hung connection.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// The release API whisker asks for prebuilt lints
///
/// This reads the environment once. It decides which API whisker asks,
/// and whether whisker holds a token for it. A run that asks about
/// several sources reuses one client, and therefore one connection
/// pool.
pub struct GitHubApi {
    base: String,
    api_host: Option<String>,
    token: Option<String>,
    client: Client,
}

/// Returns the extra remote host the configured API answers for
///
/// This reads the environment without a client, so whisker can rule a
/// remote out before it builds one.
pub fn configured_repository_host() -> Option<String> {
    repository_host(&base_from(std::env::var(API_VARIABLE).ok()))
}

impl GitHubApi {
    /// Builds the client the environment describes
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn from_environment() -> anyhow::Result<Self> {
        let base = base_from(std::env::var(API_VARIABLE).ok());
        let api_host = host_of(&base);
        let token = TOKEN_VARIABLES
            .into_iter()
            .find_map(|variable| present(std::env::var(variable).ok()));

        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("failed to build an HTTP client")?;

        Ok(Self {
            base,
            api_host,
            token,
            client,
        })
    }

    /// Returns the archive named `name` and its sidecar, if a release has them
    ///
    /// Whisker reads the first page of releases only. It misses an
    /// archive in a repository that cut a hundred releases since it
    /// published one, and the caller then compiles the source. A walk
    /// through a paginated history on every check costs more.
    ///
    /// # Errors
    ///
    /// Returns an error if the API cannot be reached or answers with
    /// anything but a listing. A repository that does not exist is not an
    /// error. Whisker sees that for a private repository it holds no
    /// token for, and the caller compiles the source instead.
    pub fn find_asset(
        &self,
        repository: &GitHubRepository,
        name: &AssetName,
    ) -> anyhow::Result<Option<PrebuiltAsset>> {
        let url = format!("{}/repos/{repository}/releases?per_page=100", self.base);

        let response = self
            .get(&url, "application/vnd.github+json")
            .with_context(|| format!("failed to ask {repository} for its releases"))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let status = response.status();
        anyhow::ensure!(
            status.is_success(),
            "the release API answered {status} for {repository}"
        );

        let body = read_text(
            response,
            MAX_LISTING,
            &format!("the releases of {repository}"),
        )?;
        let releases: Vec<Release> = serde_json::from_str(&body)
            .with_context(|| format!("failed to read the releases of {repository} as JSON"))?;

        Ok(select_asset(&releases, name))
    }

    /// Downloads `asset` to `path` and returns the digest of what arrived
    ///
    /// # Errors
    ///
    /// Returns an error if the asset cannot be fetched or written.
    pub fn download(&self, asset: &ReleaseAsset, path: &Path) -> anyhow::Result<Sha256Digest> {
        let response = self
            .get(&asset.url, "application/octet-stream")
            .with_context(|| format!("failed to download {}", asset.name))?;

        let status = response.status();
        anyhow::ensure!(
            status.is_success(),
            "the release API answered {status} for {}",
            asset.name
        );

        let mut file = std::fs::File::create(path)
            .with_context(|| format!("failed to create {}", path.display()))?;
        let written = std::io::copy(&mut response.take(MAX_ARCHIVE + 1), &mut file)
            .with_context(|| format!("failed to write {}", path.display()))?;
        drop(file);

        anyhow::ensure!(
            written <= MAX_ARCHIVE,
            "{} is larger than the {MAX_ARCHIVE} bytes whisker will download",
            asset.name
        );

        Sha256Digest::of_file(path)
    }

    /// Downloads `asset` and returns what it holds as text
    ///
    /// # Errors
    ///
    /// Returns an error if the asset cannot be fetched or is not text.
    pub fn download_text(&self, asset: &ReleaseAsset) -> anyhow::Result<String> {
        let response = self
            .get(&asset.url, "application/octet-stream")
            .with_context(|| format!("failed to download {}", asset.name))?;

        let status = response.status();
        anyhow::ensure!(
            status.is_success(),
            "the release API answered {status} for {}",
            asset.name
        );

        read_text(response, MAX_SIDECAR, &asset.name)
    }

    /// Sends one request, and carries the token only to the API's own host
    ///
    /// GitHub answers a download with a redirect to a storage host. That
    /// host signs the request itself and refuses one that also carries an
    /// `Authorization` header. The host check keeps the token off that
    /// second request, and away from anywhere whisker was not pointed.
    fn get(&self, url: &str, accept: &str) -> anyhow::Result<Response> {
        let mut request = self
            .client
            .get(url)
            .header(USER_AGENT, concat!("whisker/", env!("CARGO_PKG_VERSION")))
            .header(ACCEPT, accept);

        if let Some(token) = self.token.as_deref().filter(|_| self.serves(url)) {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }

        request.send().context("the request failed")
    }

    /// Reports whether `url` is on the host this API answers for
    fn serves(&self, url: &str) -> bool {
        let Ok(url) = reqwest::Url::parse(url) else {
            return false;
        };

        match (url.host_str(), self.api_host.as_deref()) {
            (Some(host), Some(api)) => host.eq_ignore_ascii_case(api),
            (_, _) => false,
        }
    }
}

/// One release, as much of it as whisker reads
#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
struct Release {
    #[serde(default)]
    draft: bool,

    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

/// One file attached to a release
#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
pub struct ReleaseAsset {
    name: String,

    /// The API's own address for the asset
    ///
    /// Whisker uses this rather than the browser's address, because only
    /// this one serves a private repository to a request with a token.
    url: String,
}

/// An archive of prebuilt lints and the digest published beside it
///
/// The two travel together. Whisker unpacks an archive only after it
/// checks the archive against its digest.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct PrebuiltAsset {
    pub archive: ReleaseAsset,
    pub sidecar: ReleaseAsset,
}

/// Returns the archive named `name` and its sidecar, from any release
///
/// This passes over a draft, whose assets can still change. Otherwise
/// the first release with both files wins. A name holds the commit and
/// the tag, so two releases that carry it carry the same archive.
fn select_asset(releases: &[Release], name: &AssetName) -> Option<PrebuiltAsset> {
    let sidecar = name.sidecar();

    releases
        .iter()
        .filter(|release| !release.draft)
        .find_map(|release| {
            let archive = release.assets.iter().find(|it| it.name == name.as_str())?;
            let sidecar = release.assets.iter().find(|it| it.name == sidecar)?;

            Some(PrebuiltAsset {
                archive: archive.clone(),
                sidecar: sidecar.clone(),
            })
        })
}

/// Returns the API base `value` names, without a trailing slash
///
/// A person who pastes a URL leaves a trailing slash behind, and a join
/// onto it would ask for a path with an empty segment.
fn base_from(value: Option<String>) -> String {
    let value = present(value).unwrap_or_else(|| DEFAULT_BASE.to_owned());

    value.trim_end_matches('/').to_owned()
}

/// Reads at most `cap` bytes of `source` as text
///
/// Whisker reads two things as text, and both are small: a page of
/// releases, and a line that holds a digest. This refuses a body that
/// runs past the limit.
fn read_text(source: impl Read, cap: u64, what: &str) -> anyhow::Result<String> {
    let mut body = String::new();

    source
        .take(cap + 1)
        .read_to_string(&mut body)
        .with_context(|| format!("failed to read {what}"))?;

    anyhow::ensure!(
        body.len() as u64 <= cap,
        "{what} is longer than the {cap} bytes whisker reads"
    );

    Ok(body)
}

/// Returns the host the API at `base` is reached on
///
/// This scopes the token. A request to any other host carries no
/// credential, including a request to the storage host that a download
/// redirects to.
fn host_of(base: &str) -> Option<String> {
    let url = reqwest::Url::parse(base).ok()?;

    Some(url.host_str()?.to_ascii_lowercase())
}

/// Returns the extra remote host that the API at `base` answers for
///
/// The public API lives on one host and serves repositories on another,
/// so [`GitHubRepository`] names that pair rather than derive it.
/// Anywhere else, the API and the repositories share a host, which is how
/// a GitHub Enterprise installation looks.
///
/// [`GitHubRepository`]: super::github_repository::GitHubRepository
fn repository_host(base: &str) -> Option<String> {
    if base == DEFAULT_BASE {
        return None;
    }

    host_of(base)
}

/// Returns a variable's value, and treats an empty one as unset
///
/// A shell leaves an empty variable behind when it expands something that
/// was never set. An empty token would claim an authenticated request
/// that is not one.
fn present(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GitRev;
    use crate::custom_lints::AbiTag;
    use crate::custom_lints::handshake::AbiIdentity;

    const REV: &str = "0123456789abcdef0123456789abcdef01234567";

    fn name() -> AssetName {
        let tag = AbiTag::new(
            &AbiIdentity {
                abi_version: 2,
                rustc_version: "rustc 1.92.0-nightly (0123456 2026-08-11)".to_owned(),
                types_fingerprint: 0,
                language_fingerprint: 0,
            },
            "aarch64-apple-darwin",
        );

        AssetName::new(
            &GitRev::new(REV).expect("the revision should be accepted"),
            &tag,
        )
    }

    /// Returns a client pointed at `base`, and reads no environment
    fn api_for(base: &str) -> GitHubApi {
        GitHubApi {
            base: base.to_owned(),
            api_host: host_of(base),
            token: Some("a-secret".to_owned()),
            client: Client::new(),
        }
    }

    /// Returns releases parsed from the JSON a listing would answer with
    fn releases(json: &str) -> Vec<Release> {
        serde_json::from_str(json).expect("the fixture should parse")
    }

    /// Returns a listing holding one release with `assets` attached
    fn listing(draft: bool, assets: &[&str]) -> String {
        let assets: Vec<String> = assets
            .iter()
            .map(|name| format!(r#"{{"name":"{name}","url":"https://api/{name}","size":1}}"#))
            .collect();

        format!(r#"[{{"draft":{draft},"assets":[{}]}}]"#, assets.join(","))
    }

    #[test]
    fn base_from_an_empty_value_is_the_public_api() {
        assert_eq!(base_from(Some(String::new())), DEFAULT_BASE);
    }

    #[test]
    fn base_from_no_value_is_the_public_api() {
        assert_eq!(base_from(None), DEFAULT_BASE);
    }

    #[test]
    fn base_from_a_value_drops_a_trailing_slash() {
        assert_eq!(
            base_from(Some("https://github.acme.example/api/v3/".to_owned())),
            "https://github.acme.example/api/v3"
        );
    }

    /// The token has to reach the public API too, and it once did not.
    /// The host that scoped it was the one that serves repositories, and
    /// the public API names no such host. Every request to it therefore
    /// went out unauthenticated, and no private repository answered.
    #[test]
    fn host_of_the_public_api_names_it() {
        assert_eq!(host_of(DEFAULT_BASE), Some("api.github.com".to_owned()));
    }

    #[test]
    fn host_of_another_api_names_its_host() {
        assert_eq!(
            host_of("https://github.acme.example/api/v3"),
            Some("github.acme.example".to_owned())
        );
    }

    #[test]
    fn repository_host_of_the_public_api_names_nothing() {
        assert_eq!(repository_host(DEFAULT_BASE), None);
    }

    #[test]
    fn repository_host_of_another_api_names_its_host() {
        assert_eq!(
            repository_host("https://github.acme.example/api/v3"),
            Some("github.acme.example".to_owned())
        );
    }

    /// A download redirects to a storage host. That host signs the
    /// request itself and refuses one that carries an `Authorization`
    /// header, so the token must not follow it there.
    #[test]
    fn serves_is_false_for_the_host_a_download_redirects_to() {
        let api = api_for(DEFAULT_BASE);

        assert!(!api.serves("https://objects.githubusercontent.com/whatever"));
    }

    #[test]
    fn serves_is_true_for_the_public_api() {
        let api = api_for(DEFAULT_BASE);

        assert!(api.serves("https://api.github.com/repos/aonyx-ai/rules/releases"));
    }

    #[test]
    fn serves_is_true_for_a_configured_api() {
        let api = api_for("https://github.acme.example/api/v3");

        assert!(api.serves("https://github.acme.example/api/v3/repos/team/rules/releases"));
    }

    #[test]
    fn read_text_of_a_body_within_the_limit_returns_it() {
        let body = read_text(b"digest".as_slice(), 64, "the sidecar").expect("should read");

        assert_eq!(body, "digest");
    }

    #[test]
    fn read_text_of_a_body_past_the_limit_returns_error() {
        let error =
            read_text(b"far too much".as_slice(), 4, "the sidecar").expect_err("should fail");

        assert!(format!("{error:#}").contains("longer than"), "{error:#}");
    }

    #[test]
    fn present_treats_an_empty_value_as_unset() {
        assert_eq!(present(Some(String::new())), None);
    }

    #[test]
    fn present_keeps_a_value_that_holds_something() {
        assert_eq!(present(Some("token".to_owned())), Some("token".to_owned()));
    }

    #[test]
    fn select_asset_finds_the_archive_and_its_sidecar() {
        let name = name();
        let releases = releases(&listing(false, &[name.as_str(), &name.sidecar()]));

        let asset = select_asset(&releases, &name).expect("the pair should be found");

        assert_eq!(asset.archive.name, name.as_str());
        assert_eq!(asset.sidecar.name, name.sidecar());
    }

    /// Whisker unpacks no archive it cannot check.
    #[test]
    fn select_asset_without_a_sidecar_is_none() {
        let name = name();
        let releases = releases(&listing(false, &[name.as_str()]));

        assert_eq!(select_asset(&releases, &name), None);
    }

    #[test]
    fn select_asset_without_the_archive_is_none() {
        let name = name();
        let releases = releases(&listing(
            false,
            &["another.tar.gz", "another.tar.gz.sha256"],
        ));

        assert_eq!(select_asset(&releases, &name), None);
    }

    /// Nobody published a draft, and its assets can still change.
    #[test]
    fn select_asset_skips_a_draft() {
        let name = name();
        let releases = releases(&listing(true, &[name.as_str(), &name.sidecar()]));

        assert_eq!(select_asset(&releases, &name), None);
    }

    #[test]
    fn select_asset_reads_a_later_release_when_the_first_lacks_it() {
        let name = name();
        let json = format!(
            "[{},{}]",
            listing(false, &["other.tar.gz"]).trim_matches(['[', ']']),
            listing(false, &[name.as_str(), &name.sidecar()]).trim_matches(['[', ']'])
        );
        let releases = releases(&json);

        assert!(select_asset(&releases, &name).is_some());
    }

    /// A listing carries far more than whisker reads, and GitHub adds
    /// fields over time. Neither fact may fail a check.
    #[test]
    fn releases_ignore_fields_whisker_does_not_read() {
        let json = r#"[{"id":1,"tag_name":"v1","draft":false,"prerelease":true,
            "assets":[{"name":"a","url":"https://api/a","label":null,"state":"uploaded"}]}]"#;

        let releases = releases(json);

        assert_eq!(releases.len(), 1);
    }

    #[test]
    fn releases_without_assets_parse() {
        let releases = releases(r#"[{"draft":false}]"#);

        assert!(releases[0].assets.is_empty());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GitHubApi>();
        assert_send::<PrebuiltAsset>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GitHubApi>();
        assert_sync::<PrebuiltAsset>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<GitHubApi>();
        assert_unpin::<PrebuiltAsset>();
    }
}
