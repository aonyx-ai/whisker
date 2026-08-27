//! Lint libraries that someone else already built
//!
//! A git lint source is a repository of cargo packages, and whisker turns
//! it into loadable libraries by compiling it. That costs a toolchain and
//! several minutes, and it costs them again in every project and on every
//! build agent that checks out the same pin.
//!
//! It is also the one step whisker cannot take on a machine that has no
//! matching toolchain, which a machine running a released whisker binary
//! usually has not: the handshake accepts a library only from the rustc
//! that built whisker itself.
//!
//! So whisker looks for libraries that were built once, by whoever
//! publishes the lints, against the whisker that is asking. The
//! [`AbiTag`] is how it asks. This module owns the asking and everything
//! that follows it: which remote to ask, what to call the archive it
//! wants, how to tell that the archive arrived whole, where to keep what
//! it unpacks, and which of those files are libraries.
//!
//! Nothing here is trusted for being prebuilt. Every library still goes
//! through the same handshake as one whisker compiled itself, and a
//! library that fails it is an error rather than a reason to fall back:
//! the tag is derived from exactly what the handshake compares, so a
//! mismatch means the archive was misnamed, and building from source
//! instead would hide that.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

use self::archive::Sha256Digest;
use self::asset_name::AssetName;
use self::github_release::{GitHubApi, PrebuiltAsset};
use self::github_repository::GitHubRepository;
use super::abi_tag::AbiTag;
use super::cache;
use crate::config::GitLintSource;

mod archive;
mod asset_name;
mod github_release;
mod github_repository;

/// What the downloaded archive is called while it is being checked
const DOWNLOAD: &str = "archive.tar.gz";

/// Returns the prebuilt lints for `source` that the machine already holds
///
/// Whisker keeps what it unpacks, so a run that finds its directory
/// touches no network. An empty directory does not count as a find. An
/// interrupted unpack leaves one behind, and whisker replaces it.
///
/// # Errors
///
/// Returns an error if no cache location can be determined, which is the
/// condition that also stops a git checkout.
pub fn cached(source: &GitLintSource, tag: &AbiTag) -> anyhow::Result<Option<PathBuf>> {
    let directory = cache::prebuilt_directory(source, tag)?;

    match holds_libraries(&directory) {
        true => Ok(Some(directory)),
        false => Ok(None),
    }
}

/// Asks a release for prebuilt lints, and keeps them if it has any
///
/// This is the one path here that reaches the network, so a caller asks
/// it only when the machine holds nothing that would serve instead.
///
/// Everything it does is best effort. A remote that publishes no
/// releases, a release with nothing named for this whisker, an API it
/// cannot reach, an archive that fails its digest: each one answers with
/// nothing, and the caller then compiles the source. That fallback is
/// what whisker did before any of this existed, and it is never wrong.
///
/// The cases differ in whether the reader hears about them. Whisker stays
/// quiet about a failure it cannot tell apart from a repository that
/// nobody built for.
///
/// # Errors
///
/// Returns an error if no cache location can be determined.
pub fn fetch(source: &GitLintSource, tag: &AbiTag) -> anyhow::Result<Option<PathBuf>> {
    let directory = cache::prebuilt_directory(source, tag)?;
    let host = github_release::configured_repository_host();

    let Some(repository) = GitHubRepository::from_url(source.url(), host.as_deref()) else {
        return Ok(None);
    };

    let name = AssetName::new(source.rev(), tag);

    match download(&repository, &name, &directory) {
        Ok(Installed::Yes) => Ok(Some(directory)),
        Ok(Installed::Absent) => Ok(None),
        Err(error) => {
            warn(source, &error);
            Ok(None)
        }
    }
}

/// Runs the whole exchange with the release API on a thread of its own
///
/// The HTTP client owns an asynchronous runtime, and a program panics
/// when it drops one runtime inside another. Whisker's check command is
/// asynchronous, so this thread builds the client, uses it, and drops it.
///
/// A panic on that thread becomes an ordinary failure. It would otherwise
/// vanish, and whisker would compile from source with nothing said.
fn download(
    repository: &GitHubRepository,
    name: &AssetName,
    destination: &Path,
) -> anyhow::Result<Installed> {
    std::thread::scope(|scope| {
        let thread = scope.spawn(|| {
            let api = GitHubApi::from_environment()?;

            install(&api, repository, name, destination)
        });

        match thread.join() {
            Ok(outcome) => outcome,
            Err(_) => Err(anyhow::anyhow!("the download stopped unexpectedly")),
        }
    })
}

/// Whether a release had prebuilt lints to install
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Installed {
    Yes,
    Absent,
}

/// Tells the reader that whisker compiles what it hoped to download
///
/// Whisker says this once per source, and says nothing else about the
/// prebuilt path. Nothing is broken when it appears. The check goes on
/// and reports the same diagnostics, more slowly.
fn warn(source: &GitLintSource, error: &anyhow::Error) {
    eprintln!(
        "warning: whisker cannot use the prebuilt lints for {source}: {error:#}; it builds them \
         from source instead"
    );
}

/// Downloads, checks, and unpacks the archive a release publishes
///
/// Whisker assembles the archive beside the directory it will become,
/// then moves it into place whole. A run that stops halfway therefore
/// leaves nothing a later run could mistake for a finished unpack.
///
/// Two runs may install the same archive at once. The move decides which
/// one wins, and the loser reads what the winner installed, because both
/// downloaded the same bytes.
fn install(
    api: &GitHubApi,
    repository: &GitHubRepository,
    name: &AssetName,
    destination: &Path,
) -> anyhow::Result<Installed> {
    let Some(PrebuiltAsset { archive, sidecar }) = api.find_asset(repository, name)? else {
        return Ok(Installed::Absent);
    };

    if destination.exists() {
        std::fs::remove_dir_all(destination).with_context(|| {
            format!(
                "failed to discard the damaged directory at {}",
                destination.display()
            )
        })?;
    }

    let staging = cache::staging_directory(destination);
    if staging.exists() {
        std::fs::remove_dir_all(&staging).with_context(|| {
            format!(
                "failed to discard the abandoned download at {}",
                staging.display()
            )
        })?;
    }

    let parent = destination
        .parent()
        .context("the prebuilt directory has no parent")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    std::fs::create_dir_all(&staging)
        .with_context(|| format!("failed to create {}", staging.display()))?;

    let outcome = unpack(api, &archive, &sidecar, &staging);

    if outcome.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }

    outcome.with_context(|| format!("failed to unpack {name}"))?;

    match std::fs::rename(&staging, destination) {
        Ok(()) => Ok(Installed::Yes),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);

            anyhow::ensure!(
                holds_libraries(destination),
                "failed to install the prebuilt lints at {}: {error}",
                destination.display()
            );

            Ok(Installed::Yes)
        }
    }
}

/// Downloads the archive into `staging` and unpacks what it holds
///
/// This compares the digest before it unpacks anything, so a truncated or
/// corrupted download never becomes files. It then removes the archive,
/// because whisker installs the libraries and nothing else.
fn unpack(
    api: &GitHubApi,
    archive: &github_release::ReleaseAsset,
    sidecar: &github_release::ReleaseAsset,
    staging: &Path,
) -> anyhow::Result<()> {
    let downloaded = staging.join(DOWNLOAD);

    let published = api.download_text(sidecar)?;
    let published = Sha256Digest::from_sidecar(&published)?;
    let arrived = api.download(archive, &downloaded)?;

    anyhow::ensure!(
        arrived == published,
        "the archive has digest {arrived}, but the sidecar beside it publishes {published}"
    );

    archive::extract(&downloaded, staging)?;

    std::fs::remove_file(&downloaded)
        .with_context(|| format!("failed to remove {}", downloaded.display()))?;

    anyhow::ensure!(
        holds_libraries(staging),
        "the archive holds no dynamic library at its root"
    );

    Ok(())
}

/// Returns every dynamic library directly inside `directory`, in order
///
/// A publisher ships one library per lint package, so a directory holds
/// as many as the repository built. Whisker ignores every other file, so
/// a publisher may add a manifest or a license beside them.
///
/// The order is the file names', so that two runs load the same lints in
/// the same order and report their diagnostics the same way.
///
/// # Errors
///
/// Returns an error if the directory cannot be read.
pub fn libraries(directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?;

    let mut libraries = Vec::new();

    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read an entry of {}", directory.display()))?;
        let path = entry.path();

        if path.extension() != Some(OsStr::new(std::env::consts::DLL_EXTENSION)) {
            continue;
        }

        if !path.is_file() {
            continue;
        }

        libraries.push(path);
    }

    libraries.sort();

    Ok(libraries)
}

/// Returns whether `directory` holds at least one loadable library
///
/// A directory whisker cannot read counts as empty. Whisker replaces
/// such a cache entry, and a run continues.
fn holds_libraries(directory: &Path) -> bool {
    libraries(directory).is_ok_and(|libraries| !libraries.is_empty())
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tempfile::TempDir;

    use super::*;

    /// Writes `names` as empty files in a new directory
    fn directory_holding(names: &[&str]) -> TempDir {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        for name in names {
            File::create(directory.path().join(name)).expect("the file should be created");
        }

        directory
    }

    /// Returns a file name that whisker would load on this platform
    fn library(stem: &str) -> String {
        format!(
            "{}{stem}.{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_EXTENSION
        )
    }

    #[test]
    fn holds_libraries_with_a_library_is_true() {
        let directory = directory_holding(&[&library("one")]);

        assert!(holds_libraries(directory.path()));
    }

    #[test]
    fn holds_libraries_with_nothing_to_load_is_false() {
        let directory = directory_holding(&["README.md"]);

        assert!(!holds_libraries(directory.path()));
    }

    /// An unpack that was interrupted leaves a directory behind, and the
    /// next run has to be able to replace it rather than fail on it.
    #[test]
    fn holds_libraries_of_a_missing_directory_is_false() {
        let directory = directory_holding(&[]);
        let missing = directory.path().join("absent");

        assert!(!holds_libraries(&missing));
    }

    #[test]
    fn libraries_are_ordered_by_name() {
        let directory = directory_holding(&[&library("b"), &library("a"), &library("c")]);

        let libraries = libraries(directory.path()).expect("the directory should be read");

        let names: Vec<_> = libraries
            .iter()
            .filter_map(|path| path.file_name())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();

        assert_eq!(names, sorted);
    }

    #[test]
    fn libraries_ignores_a_directory_that_looks_like_one() {
        let directory = directory_holding(&[&library("real")]);
        std::fs::create_dir(directory.path().join(library("fake")))
            .expect("the directory should be created");

        let libraries = libraries(directory.path()).expect("the directory should be read");

        assert_eq!(libraries.len(), 1, "{libraries:?}");
    }

    #[test]
    fn libraries_ignores_everything_that_is_not_a_library() {
        let directory = directory_holding(&[&library("one"), "README.md", "notes.txt", "plain"]);

        let libraries = libraries(directory.path()).expect("the directory should be read");

        assert_eq!(libraries.len(), 1, "{libraries:?}");
    }

    #[test]
    fn libraries_of_an_empty_directory_is_empty() {
        let directory = directory_holding(&[]);

        let libraries = libraries(directory.path()).expect("the directory should be read");

        assert!(libraries.is_empty(), "{libraries:?}");
    }

    #[test]
    fn libraries_of_a_missing_directory_returns_error() {
        let directory = directory_holding(&[]);
        let missing = directory.path().join("absent");

        let error = libraries(&missing).expect_err("should fail");

        assert!(format!("{error:#}").contains("absent"), "{error:#}");
    }
}
