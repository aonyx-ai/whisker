use std::path::Path;

use super::check_outcome::CheckOutcome;

/// What `whisker check` does with a file it cannot read or analyze
///
/// Read failures and analysis failures both go through
/// [`ErrorRecovery::record`], so one place decides whether a per-file
/// failure ends the run. The `--keep-going` flag selects
/// [`ErrorRecovery::Continue`].
// r[impl cli.check.keep-going]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum ErrorRecovery {
    /// The first failing file ends the run with its error
    Abort,
    /// Whisker reports each failing file, continues, and still fails the run
    Continue,
}

impl ErrorRecovery {
    /// Records a failure for one file
    ///
    /// Under [`ErrorRecovery::Continue`], the error goes to stderr and
    /// `outcome` becomes [`CheckOutcome::Failure`].
    ///
    /// # Errors
    ///
    /// Under [`ErrorRecovery::Abort`], returns `error` with the file name
    /// added, so the caller can end the run with it.
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
