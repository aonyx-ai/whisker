use std::path::PathBuf;

use crate::config::GitLintSource;

mod cache;

/// Returns the checkout the cache holds for `source`
///
/// A [`GitRev`] names one immutable commit, so a checkout that exists is
/// the right one forever and whisker reuses it without asking the remote
/// anything. That is what keeps a check working on a train, and it is
/// also why the cache never expires: there is no later version of a
/// commit to miss.
///
/// Fetching a checkout that is not there yet follows in its own change.
///
/// # Errors
///
/// Returns an error if no cache location can be determined, or if the
/// cache holds no checkout of `source`.
///
/// [`GitRev`]: crate::config::GitRev
pub fn checkout(source: &GitLintSource) -> anyhow::Result<PathBuf> {
    let destination = cache::checkout_directory(source)?;

    anyhow::ensure!(
        destination.is_dir(),
        "the cache holds no checkout of {source} at {}, and whisker cannot fetch one yet",
        destination.display()
    );

    Ok(destination)
}
