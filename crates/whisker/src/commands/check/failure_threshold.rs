use whisker_types::{Diagnostic, Severity};

/// The lowest diagnostic severity that makes `whisker check` fail
///
/// The default is [`FailureThreshold::Errors`]. The `--deny-warnings` flag
/// selects [`FailureThreshold::Warnings`].
// r[impl cli.check.deny-warnings]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum FailureThreshold {
    /// Only [`Severity::Error`] diagnostics fail the run
    ///
    /// [`Severity::Error`]: whisker_types::Severity::Error
    Errors,
    /// [`Severity::Warn`] diagnostics fail the run alongside errors
    ///
    /// [`Severity::Warn`]: whisker_types::Severity::Warn
    Warnings,
}

impl FailureThreshold {
    /// Returns whether a diagnostic of this severity fails the run
    pub(crate) fn is_met_by(self, severity: Severity) -> bool {
        severity >= self.minimum_failing_severity()
    }

    /// Returns the lowest severity that fails the run under this threshold
    pub(crate) fn minimum_failing_severity(self) -> Severity {
        match self {
            Self::Errors => Severity::Error,
            Self::Warnings => Severity::Warn,
        }
    }

    /// Raises a failing diagnostic to error severity
    ///
    /// A diagnostic that meets the threshold gets [`Severity::Error`], so
    /// the rendered output matches the exit code. A diagnostic below the
    /// threshold comes back unchanged.
    ///
    /// [`Severity::Error`]: whisker_types::Severity::Error
    pub(crate) fn promote(self, diagnostic: Diagnostic) -> Diagnostic {
        match self.is_met_by(diagnostic.severity()) {
            true => diagnostic.with_severity(Severity::Error),
            false => diagnostic,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{Location, RuleId, Span, Suggestion};

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
        assert_send::<FailureThreshold>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FailureThreshold>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FailureThreshold>();
    }

    #[test]
    fn is_met_by_with_errors_accepts_only_error() {
        let threshold = FailureThreshold::Errors;

        assert!(!threshold.is_met_by(Severity::Help));
        assert!(!threshold.is_met_by(Severity::Info));
        assert!(!threshold.is_met_by(Severity::Warn));
        assert!(threshold.is_met_by(Severity::Error));
    }

    #[test]
    fn is_met_by_with_warnings_accepts_warn_and_error() {
        let threshold = FailureThreshold::Warnings;

        assert!(!threshold.is_met_by(Severity::Help));
        assert!(!threshold.is_met_by(Severity::Info));
        assert!(threshold.is_met_by(Severity::Warn));
        assert!(threshold.is_met_by(Severity::Error));
    }

    #[test]
    fn minimum_failing_severity_with_errors_returns_error() {
        let severity = FailureThreshold::Errors.minimum_failing_severity();

        assert_eq!(severity, Severity::Error);
    }

    #[test]
    fn minimum_failing_severity_with_warnings_returns_warn() {
        let severity = FailureThreshold::Warnings.minimum_failing_severity();

        assert_eq!(severity, Severity::Warn);
    }

    #[test]
    fn promote_with_errors_leaves_every_severity_untouched() {
        let severities = [
            Severity::Help,
            Severity::Info,
            Severity::Warn,
            Severity::Error,
        ];

        let promoted: Vec<Severity> = severities
            .into_iter()
            .map(|severity| {
                FailureThreshold::Errors
                    .promote(test_diagnostic(severity))
                    .severity()
            })
            .collect();

        assert_eq!(promoted, severities);
    }

    #[test]
    fn promote_with_warnings_leaves_help_and_info_untouched() {
        let help = FailureThreshold::Warnings.promote(test_diagnostic(Severity::Help));
        let info = FailureThreshold::Warnings.promote(test_diagnostic(Severity::Info));

        assert_eq!(help.severity(), Severity::Help);
        assert_eq!(info.severity(), Severity::Info);
    }

    #[test]
    fn promote_with_warnings_preserves_diagnostic_details() {
        let diagnostic = test_diagnostic(Severity::Warn)
            .with_origin(Location::new(
                Span::new(PathBuf::from("other.rs"), 1, 2),
                "defined here".into(),
            ))
            .with_related(Location::new(
                Span::new(PathBuf::from("other.rs"), 3, 4),
                "also here".into(),
            ))
            .with_suggestion(Suggestion::new(
                Span::new(PathBuf::from("test.rs"), 0, 1),
                "replacement".into(),
                "try this".into(),
            ));

        let promoted = FailureThreshold::Warnings.promote(diagnostic);

        assert_eq!(promoted.rule_id(), RuleId("lint.test"));
        assert_eq!(promoted.message(), "test message");
        assert_eq!(promoted.span().start(), 0);
        assert_eq!(promoted.span().end(), 10);
        assert_eq!(promoted.origins().len(), 1);
        assert_eq!(promoted.related().len(), 1);
        assert_eq!(promoted.suggestions().len(), 1);
    }

    #[test]
    fn promote_with_warnings_raises_warn_to_error() {
        let promoted = FailureThreshold::Warnings.promote(test_diagnostic(Severity::Warn));

        assert_eq!(promoted.severity(), Severity::Error);
    }
}
