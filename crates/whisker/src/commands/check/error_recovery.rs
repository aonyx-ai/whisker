use std::path::Path;

use super::check_outcome::CheckOutcome;

/// What `whisker check` does with a file it cannot read or analyze
///
/// A single file can fail for two unrelated reasons — the bytes on disk are
/// not readable as UTF-8, or the pipeline rejects the source it read — and
/// the run has to treat both the same way. Routing them through one type
/// means there is exactly one place that decides whether a per-file failure
/// ends the run, so the two paths cannot drift apart or, worse, disagree
/// about whether the failure reaches the exit code.
///
/// Under [`ErrorRecovery::Continue`] the failure is reported and the walk
/// moves on, which is the only mode where the recorded
/// [`CheckOutcome::Failure`] matters: a file that failed to load produces no
/// diagnostics, so nothing else would keep the exit code non-zero.
///
/// The `--keep-going` flag arrives as a [`bool`] because that is what clap
/// parses at the command line boundary. The command converts it as soon as
/// it destructures the parsed arguments, so no part of the program below
/// that boundary has to work out what a bare boolean means.
// r[impl cli.check.keep-going]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum ErrorRecovery {
    /// The first failing file ends the run and its error is returned
    Abort,
    /// A failing file is reported, the run continues, and the run still fails
    Continue,
}

impl ErrorRecovery {
    /// Records a failure encountered while processing a single file
    ///
    /// Under [`ErrorRecovery::Continue`] the error is written to stderr and
    /// `outcome` is driven to [`CheckOutcome::Failure`], which is what folds
    /// the failure into the exit code once the walk finishes.
    ///
    /// # Errors
    ///
    /// Returns `error` annotated with the file it came from when this is
    /// [`ErrorRecovery::Abort`], so the caller can end the run with it.
    pub(crate) fn record(
        self,
        outcome: &mut CheckOutcome,
        file: &Path,
        error: anyhow::Error,
    ) -> anyhow::Result<()> {
        match self {
            Self::Abort => Err(error.context(format!("check {}", file.display()))),
            Self::Continue => {
                eprintln!("error: {}: {error:#}", file.display());
                *outcome = CheckOutcome::Failure;
                Ok(())
            }
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
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ErrorRecovery>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ErrorRecovery>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ErrorRecovery>();
    }

    #[test]
    fn record_with_abort_leaves_the_outcome_untouched() {
        let mut outcome = CheckOutcome::Success;

        let _error = ErrorRecovery::Abort
            .record(&mut outcome, Path::new("bad.rs"), test_error())
            .expect_err("abort should fail the run");

        assert_eq!(outcome, CheckOutcome::Success);
    }

    #[test]
    fn record_with_abort_names_the_failing_file() {
        let mut outcome = CheckOutcome::Success;

        let error = ErrorRecovery::Abort
            .record(&mut outcome, Path::new("bad.rs"), test_error())
            .expect_err("abort should fail the run");

        assert_eq!(format!("{error:#}"), "check bad.rs: something broke");
    }

    #[test]
    fn record_with_continue_keeps_an_earlier_failure() {
        let mut outcome = CheckOutcome::Failure;

        ErrorRecovery::Continue
            .record(&mut outcome, Path::new("bad.rs"), test_error())
            .expect("continue should not fail the run");

        assert_eq!(outcome, CheckOutcome::Failure);
    }

    #[test]
    fn record_with_continue_marks_the_outcome_failed() {
        let mut outcome = CheckOutcome::Success;

        ErrorRecovery::Continue
            .record(&mut outcome, Path::new("bad.rs"), test_error())
            .expect("continue should not fail the run");

        assert_eq!(outcome, CheckOutcome::Failure);
    }
}
