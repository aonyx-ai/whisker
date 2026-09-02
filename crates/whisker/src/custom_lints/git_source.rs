use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::Context as _;

use super::cache;
use crate::config::{GitLintSource, GitRev, GitUrl};

/// Returns a checkout of `source`, fetching it if the cache lacks one
///
/// A checkout is permanent once it exists, because a [`GitRev`] names one
/// immutable commit: the same source can never want different content
/// later. A run that finds its checkout therefore touches no network at
/// all, which keeps a check working on a train and keeps the common case
/// as fast as a local path.
///
/// Two runs may fetch the same pin at once. The rename into place decides
/// which of them wins, and the loser discards its own work and reads what
/// the winner installed, because both fetched the same commit.
///
/// # Errors
///
/// Returns an error if the cache directory cannot be created, if the
/// remote cannot be reached, if it does not serve the pinned commit, or if
/// the working tree cannot be written.
pub fn checkout(source: &GitLintSource) -> anyhow::Result<PathBuf> {
    let destination = cache::checkout_directory(source)?;

    if holds(&destination, source.rev()) {
        return Ok(destination);
    }

    if destination.exists() {
        std::fs::remove_dir_all(&destination).with_context(|| {
            format!(
                "failed to discard the damaged checkout at {}",
                destination.display()
            )
        })?;
    }

    let staging = cache::staging_directory(&destination);
    if staging.exists() {
        std::fs::remove_dir_all(&staging).with_context(|| {
            format!(
                "failed to discard the abandoned checkout at {}",
                staging.display()
            )
        })?;
    }

    let parent = destination
        .parent()
        .context("the checkout directory has no parent")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;

    materialize(source, &staging)
        .with_context(|| format!("failed to check out {} from {}", source.rev(), source.url()))?;

    match std::fs::rename(&staging, &destination) {
        Ok(()) => Ok(destination),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);

            anyhow::ensure!(
                holds(&destination, source.rev()),
                "failed to install the checkout at {}: {error}",
                destination.display()
            );

            Ok(destination)
        }
    }
}

/// Reports whether `directory` is a repository checked out at `rev`
///
/// The question is asked of git rather than of the filesystem, because a
/// directory can exist for reasons a fetch never intended: a half-copied
/// backup, or a checkout of some other commit. Anything that does not
/// answer with the pinned commit is treated as absent and refetched, since
/// a wrong tree would silently run the wrong rules.
///
/// What this does not detect is an edit to a file the checkout already
/// holds, which leaves `HEAD` answering correctly. A fetch installs a
/// finished tree with a rename, so whisker never publishes a half-written
/// checkout of its own.
fn holds(directory: &Path, rev: &GitRev) -> bool {
    let Ok(repository) = gix::open_opts(directory, open_options()) else {
        return false;
    };

    let Ok(head) = repository.head_id() else {
        return false;
    };

    let Ok(pinned) = gix::ObjectId::from_hex(rev.as_str().as_bytes()) else {
        return false;
    };

    head == pinned
}

/// Returns how whisker opens a repository it keeps in its own cache
///
/// A checkout in the cache belongs to whisker rather than to whatever
/// invoked it. Whisker may run from inside a git hook, which exports
/// `GIT_DIR` and `GIT_INDEX_FILE` pointing at the repository being
/// committed, and gitoxide reads them. The `GIT_*` category is therefore
/// denied, so a checkout cannot be redirected onto somebody else's
/// repository.
///
/// Every other category stays open. A private rule repository is fetched
/// with the credentials and the transport settings that the home directory
/// and the environment carry, and denying those would only turn a working
/// fetch into a failing one.
fn open_options() -> gix::open::Options {
    let permissions = gix::open::Permissions {
        env: gix::open::permissions::Environment {
            git_prefix: gix::sec::Permission::Deny,
            ..gix::open::permissions::Environment::all()
        },
        ..gix::open::Permissions::default()
    };

    gix::open::Options::default().permissions(permissions)
}

/// Fetches the pinned commit into `staging` and writes its working tree
///
/// # Errors
///
/// Returns an error if the repository cannot be created, the fetch fails,
/// the remote does not serve the commit, or the checkout cannot be written.
fn materialize(source: &GitLintSource, staging: &Path) -> anyhow::Result<()> {
    let repository = gix::ThreadSafeRepository::init_opts(
        staging,
        gix::create::Kind::WithWorktree,
        gix::create::Options::default(),
        open_options(),
    )
    .with_context(|| format!("failed to create a repository at {}", staging.display()))?
    .to_thread_local();

    fetch(&repository, source.url(), source.rev())?;

    let commit = gix::ObjectId::from_hex(source.rev().as_str().as_bytes())
        .with_context(|| format!("failed to read {} as a commit hash", source.rev()))?;
    let tree = repository
        .find_object(commit)
        .with_context(|| {
            format!(
                "{} does not serve the commit {}",
                source.url(),
                source.rev()
            )
        })?
        .peel_to_commit()
        .with_context(|| format!("{} is not a commit", source.rev()))?
        .tree_id()
        .context("failed to read the commit's tree")?;

    let mut index = repository
        .index_from_tree(&tree)
        .context("failed to build an index from the commit's tree")?;
    let workdir = repository
        .workdir()
        .context("the fetched repository has no working tree")?
        .to_path_buf();

    gix::worktree::state::checkout(
        &mut index,
        workdir,
        repository.objects.clone(),
        &gix::progress::Discard,
        &gix::progress::Discard,
        &AtomicBool::default(),
        gix::worktree::state::checkout::Options::default(),
    )
    .context("failed to write the working tree")?;

    index
        .write(gix::index::write::Options::default())
        .context("failed to write the index")?;

    detach_head(&repository, commit)?;

    Ok(())
}

/// Points the checkout's `HEAD` at the commit it was built from
///
/// This is what lets a later run recognize the checkout as the pin it
/// already has, and it leaves the cache full of repositories a person can
/// inspect with ordinary git.
///
/// The file is written rather than edited through a ref transaction,
/// because a transaction also writes a reflog, and a reflog entry needs a
/// committer. Whisker would then be unable to fetch on any machine whose
/// git identity is unset, which is most build agents, and would fail there
/// with an error about `user.email` that has nothing to do with linting. A
/// detached `HEAD` is a file holding the hash, and this checkout has no
/// history worth logging.
///
/// # Errors
///
/// Returns an error if the file cannot be written.
fn detach_head(repository: &gix::Repository, commit: gix::ObjectId) -> anyhow::Result<()> {
    let head = repository.path().join("HEAD");

    std::fs::write(&head, format!("{commit}\n"))
        .with_context(|| format!("failed to write {}", head.display()))?;

    Ok(())
}

/// Asks the remote for the pinned commit and nothing else
///
/// The refspec names the commit itself rather than a branch, so the remote
/// sends the one history whisker asked for even when no branch tip matches
/// it. Depth one cuts that history to a single commit, because the lints
/// are built from the tree, never from what came before it. Tags are
/// declined for the same reason.
///
/// # Errors
///
/// Returns an error if the remote cannot be reached, refuses to serve a
/// commit by hash, or does not have it.
fn fetch(repository: &gix::Repository, url: &GitUrl, rev: &GitRev) -> anyhow::Result<()> {
    let refspec = format!("+{rev}:refs/commit/{rev}");

    let remote = repository
        .remote_at(url.to_gix_url())
        .with_context(|| format!("failed to build a remote for {url}"))?
        .with_fetch_tags(gix::remote::fetch::Tags::None)
        .with_refspecs([refspec.as_bytes()], gix::remote::Direction::Fetch)
        .context("failed to build the fetch refspec")?;

    let connection = remote
        .connect(gix::remote::Direction::Fetch)
        .with_context(|| format!("failed to connect to {url}"))?;

    connection
        .prepare_fetch(
            gix::progress::Discard,
            gix::remote::ref_map::Options::default(),
        )
        .with_context(|| format!("failed to negotiate a fetch with {url}"))?
        .with_shallow(gix::remote::fetch::Shallow::DepthAtRemote(
            1.try_into().expect("one is not zero"),
        ))
        .receive(gix::progress::Discard, &AtomicBool::default())
        .with_context(|| format!("failed to fetch {rev} from {url}"))?;

    Ok(())
}
