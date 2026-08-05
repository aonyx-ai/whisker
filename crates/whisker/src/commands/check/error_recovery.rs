use std::path::Path;

use super::check_outcome::CheckOutcome;

/// Whether the walk carries on after a file failed
///
/// A per-file failure never propagates out of the walk. Whichever mode is in
/// force, the run has already collected diagnostics from the files it got
/// through, and those findings are true regardless of what happened next;
/// discarding them would make the user pay twice for one bad file. So the
/// only thing left to decide is whether to keep walking.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum Walk {
    /// The walk moves on to the next file
    Continue,
    /// The walk stops, and the run reports what it already collected
    Stop,
}

/// What `whisker check` does with a file it cannot read or analyze
///
/// A single file can fail for two unrelated reasons — the bytes on disk are
/// not readable as UTF-8, or the pipeline rejects the source it read — and
/// the run has to treat both the same way. Routing them through one type
/// means there is exactly one place that decides whether a per-file failure
/// ends the run, so the two paths cannot drift apart or, worse, disagree
/// about whether the failure reaches the exit code.
///
/// Both modes report the failure and drive `outcome` to
/// [`CheckOutcome::Failure`], because a file that failed to load produces no
/// diagnostics and nothing else would keep the exit code non-zero. They
/// differ only in whether the remaining files are walked.
///
/// The `--keep-going` flag arrives as a [`bool`] because that is what clap
/// parses at the command line boundary. The command converts it as soon as
/// it destructures the parsed arguments, so no part of the program below
/// that boundary has to work out what a bare boolean means.
// r[impl cli.check.keep-going]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum ErrorRecovery {
    /// The first failing file ends the walk
    Abort,
    /// A failing file is reported and the walk carries on
    Continue,
}

impl ErrorRecovery {
    /// Reports a failure encountered while processing a single file
    ///
    /// The error is written to stderr and `outcome` is driven to
    /// [`CheckOutcome::Failure`], which is what folds the failure into the
    /// exit code once the walk finishes.
    ///
    /// Returns whether the caller should keep walking.
    pub(crate) fn record(
        self,
        outcome: &mut CheckOutcome,
        file: &Path,
        error: anyhow::Error,
    ) -> Walk {
        eprintln!("error: {}: {error:#}", file.display());
        *outcome = CheckOutcome::Failure;

        match self {
            Self::Abort => Walk::Stop,
            Self::Continue => Walk::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_error() -> anyhow::Error {
        anyhow::anyhow!("something broke")
    }

    #[test]
    fn record_with_abort_fails_the_outcome() {
        let mut outcome = CheckOutcome::Success;

        ErrorRecovery::Abort.record(&mut outcome, Path::new("bad.rs"), test_error());

        assert_eq!(outcome, CheckOutcome::Failure);
    }

    #[test]
    fn record_with_abort_stops_the_walk() {
        let mut outcome = CheckOutcome::Success;

        let walk = ErrorRecovery::Abort.record(&mut outcome, Path::new("bad.rs"), test_error());

        assert_eq!(walk, Walk::Stop);
    }

    #[test]
    fn record_with_continue_fails_the_outcome() {
        let mut outcome = CheckOutcome::Success;

        ErrorRecovery::Continue.record(&mut outcome, Path::new("bad.rs"), test_error());

        assert_eq!(outcome, CheckOutcome::Failure);
    }

    #[test]
    fn record_with_continue_keeps_an_earlier_failure() {
        let mut outcome = CheckOutcome::Failure;

        ErrorRecovery::Continue.record(&mut outcome, Path::new("bad.rs"), test_error());

        assert_eq!(outcome, CheckOutcome::Failure);
    }

    #[test]
    fn record_with_continue_keeps_walking() {
        let mut outcome = CheckOutcome::Success;

        let walk = ErrorRecovery::Continue.record(&mut outcome, Path::new("bad.rs"), test_error());

        assert_eq!(walk, Walk::Continue);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ErrorRecovery>();
        assert_send::<Walk>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ErrorRecovery>();
        assert_sync::<Walk>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ErrorRecovery>();
        assert_unpin::<Walk>();
    }
}
