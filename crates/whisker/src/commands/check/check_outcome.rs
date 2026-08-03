use whisker_types::Diagnostic;

use super::failure_threshold::FailureThreshold;

/// Whether a `whisker check` run should report success or failure
///
/// The run's verdict is computed as a value rather than by calling
/// [`std::process::exit`] at the point each problem is discovered. A run
/// fails for two independent reasons — a file that could not be read or
/// analyzed, and a diagnostic severe enough to meet the configured
/// [`FailureThreshold`] — and both are folded into one outcome so the
/// command has a single place where it turns the verdict into an exit code.
/// Keeping the decision out of the process-exiting code is what makes it
/// testable.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum CheckOutcome {
    /// Nothing worth failing the build over was found
    Success,
    /// At least one failure was found and the process must exit non-zero
    Failure,
}

impl CheckOutcome {
    /// Returns the outcome implied by a set of diagnostics
    ///
    /// The diagnostics may be given before or after promotion by the same
    /// threshold: promotion only ever raises a diagnostic that already met
    /// the threshold, so the verdict is the same either way.
    // r[impl cli.diagnostics.exit-code]
    pub(crate) fn from_diagnostics(
        diagnostics: &[Diagnostic],
        threshold: FailureThreshold,
    ) -> Self {
        let failed = diagnostics
            .iter()
            .any(|diagnostic| threshold.is_met_by(diagnostic.severity()));

        match failed {
            true => Self::Failure,
            false => Self::Success,
        }
    }

    /// Combines two outcomes, failing if either of them failed
    pub(crate) fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Success, Self::Success) => Self::Success,
            (Self::Success, Self::Failure) => Self::Failure,
            (Self::Failure, Self::Success) => Self::Failure,
            (Self::Failure, Self::Failure) => Self::Failure,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{RuleId, Severity, Span};

    use super::*;

    fn test_diagnostic(severity: Severity) -> Diagnostic {
        Diagnostic::new(
            RuleId("lint.test"),
            severity,
            "test message".into(),
            Span::new(PathBuf::from("test.rs"), 0, 10),
        )
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CheckOutcome>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CheckOutcome>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CheckOutcome>();
    }

    #[test]
    fn combine_with_any_failure_returns_failure() {
        assert_eq!(
            CheckOutcome::Success.combine(CheckOutcome::Failure),
            CheckOutcome::Failure
        );
        assert_eq!(
            CheckOutcome::Failure.combine(CheckOutcome::Success),
            CheckOutcome::Failure
        );
        assert_eq!(
            CheckOutcome::Failure.combine(CheckOutcome::Failure),
            CheckOutcome::Failure
        );
    }

    #[test]
    fn combine_with_two_successes_returns_success() {
        let outcome = CheckOutcome::Success.combine(CheckOutcome::Success);

        assert_eq!(outcome, CheckOutcome::Success);
    }

    #[test]
    fn from_diagnostics_with_error_and_errors_returns_failure() {
        let diagnostics = [test_diagnostic(Severity::Error)];

        let outcome = CheckOutcome::from_diagnostics(&diagnostics, FailureThreshold::Errors);

        assert_eq!(outcome, CheckOutcome::Failure);
    }

    #[test]
    fn from_diagnostics_with_error_and_warnings_returns_failure() {
        let diagnostics = [test_diagnostic(Severity::Error)];

        let outcome = CheckOutcome::from_diagnostics(&diagnostics, FailureThreshold::Warnings);

        assert_eq!(outcome, CheckOutcome::Failure);
    }

    #[test]
    fn from_diagnostics_with_help_and_info_never_returns_failure() {
        let diagnostics = [
            test_diagnostic(Severity::Help),
            test_diagnostic(Severity::Info),
        ];

        let lenient = CheckOutcome::from_diagnostics(&diagnostics, FailureThreshold::Errors);
        let strict = CheckOutcome::from_diagnostics(&diagnostics, FailureThreshold::Warnings);

        assert_eq!(lenient, CheckOutcome::Success);
        assert_eq!(strict, CheckOutcome::Success);
    }

    #[test]
    fn from_diagnostics_with_mixed_severities_and_errors_returns_failure() {
        let diagnostics = [
            test_diagnostic(Severity::Help),
            test_diagnostic(Severity::Warn),
            test_diagnostic(Severity::Error),
        ];

        let outcome = CheckOutcome::from_diagnostics(&diagnostics, FailureThreshold::Errors);

        assert_eq!(outcome, CheckOutcome::Failure);
    }

    #[test]
    fn from_diagnostics_with_no_diagnostics_returns_success() {
        let lenient = CheckOutcome::from_diagnostics(&[], FailureThreshold::Errors);
        let strict = CheckOutcome::from_diagnostics(&[], FailureThreshold::Warnings);

        assert_eq!(lenient, CheckOutcome::Success);
        assert_eq!(strict, CheckOutcome::Success);
    }

    #[test]
    fn from_diagnostics_with_promoted_warnings_returns_failure() {
        let diagnostics: Vec<Diagnostic> = [test_diagnostic(Severity::Warn)]
            .into_iter()
            .map(|diagnostic| FailureThreshold::Warnings.promote(diagnostic))
            .collect();

        let outcome = CheckOutcome::from_diagnostics(&diagnostics, FailureThreshold::Warnings);

        assert_eq!(outcome, CheckOutcome::Failure);
    }

    // r[verify cli.check.deny-warnings]
    #[test]
    fn from_diagnostics_with_warning_and_errors_returns_success() {
        let diagnostics = [test_diagnostic(Severity::Warn)];

        let outcome = CheckOutcome::from_diagnostics(&diagnostics, FailureThreshold::Errors);

        assert_eq!(outcome, CheckOutcome::Success);
    }

    // r[verify cli.check.deny-warnings]
    #[test]
    fn from_diagnostics_with_warning_and_warnings_returns_failure() {
        let diagnostics = [test_diagnostic(Severity::Warn)];

        let outcome = CheckOutcome::from_diagnostics(&diagnostics, FailureThreshold::Warnings);

        assert_eq!(outcome, CheckOutcome::Failure);
    }
}
