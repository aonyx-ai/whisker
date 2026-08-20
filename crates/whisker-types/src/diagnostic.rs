use crate::{Location, RuleId, Severity, Span, Suggestion};

/// A diagnostic emitted by a lint rule
///
/// Diagnostics are the primary output of the linting pipeline. Each one
/// identifies a specific issue found in source code, with a severity, a
/// primary span, and optional supplementary information like origin
/// locations, related locations, and suggested fixes.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    rule_id: RuleId,
    severity: Severity,
    message: String,
    span: Span,
    origins: Vec<Location>,
    related: Vec<Location>,
    suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    /// Creates a new diagnostic with the required fields
    pub fn new(rule_id: RuleId, severity: Severity, message: String, span: Span) -> Self {
        Self {
            rule_id,
            severity,
            message,
            span,
            origins: Vec::new(),
            related: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Adds an origin location to this diagnostic
    pub fn with_origin(mut self, origin: Location) -> Self {
        self.origins.push(origin);
        self
    }

    /// Adds a related location to this diagnostic
    pub fn with_related(mut self, related: Location) -> Self {
        self.related.push(related);
        self
    }

    /// Adds a suggested fix to this diagnostic
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Replaces the severity of this diagnostic
    ///
    /// The other `with_*` methods append. This one replaces, because a
    /// diagnostic has exactly one severity.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// use whisker_types::{Diagnostic, RuleId, Severity, Span};
    ///
    /// let diagnostic = Diagnostic::new(
    ///     RuleId::new("lint.bool-param"),
    ///     Severity::Warn,
    ///     "parameter has type `bool`".into(),
    ///     Span::new(PathBuf::from("src/lib.rs"), 0, 10),
    /// );
    ///
    /// let denied = diagnostic.with_severity(Severity::Error);
    ///
    /// assert_eq!(denied.severity(), Severity::Error);
    /// ```
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Returns the rule that produced this diagnostic
    pub fn rule_id(&self) -> RuleId {
        self.rule_id
    }

    /// Returns the severity of this diagnostic
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the human-readable diagnostic message
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the primary source span
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns origin locations that explain where something was defined
    pub fn origins(&self) -> &[Location] {
        &self.origins
    }

    /// Returns related locations that provide additional context
    pub fn related(&self) -> &[Location] {
        &self.related
    }

    /// Returns suggested source code fixes
    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn test_diagnostic() -> Diagnostic {
        Diagnostic::new(
            RuleId::new("lint.test"),
            Severity::Warn,
            "test message".into(),
            Span::new(PathBuf::from("test.rs"), 0, 10),
        )
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Diagnostic>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Diagnostic>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Diagnostic>();
    }

    #[test]
    fn accessors_return_correct_values() {
        let diag = test_diagnostic();

        assert_eq!(diag.rule_id(), RuleId::new("lint.test"));
        assert_eq!(diag.severity(), Severity::Warn);
        assert_eq!(diag.message(), "test message");
        assert_eq!(diag.span().start(), 0);
        assert_eq!(diag.span().end(), 10);
    }

    #[test]
    fn new_diagnostic_has_empty_collections() {
        let diag = test_diagnostic();

        assert!(diag.origins().is_empty());
        assert!(diag.related().is_empty());
        assert!(diag.suggestions().is_empty());
    }

    #[test]
    fn with_origin_adds_origin() {
        let origin = Location::new(
            Span::new(PathBuf::from("other.rs"), 5, 15),
            "defined here".into(),
        );
        let diag = test_diagnostic().with_origin(origin);

        assert_eq!(diag.origins().len(), 1);
        assert_eq!(diag.origins()[0].message(), "defined here");
    }

    #[test]
    fn with_related_adds_related() {
        let related = Location::new(
            Span::new(PathBuf::from("other.rs"), 5, 15),
            "also used here".into(),
        );
        let diag = test_diagnostic().with_related(related);

        assert_eq!(diag.related().len(), 1);
        assert_eq!(diag.related()[0].message(), "also used here");
    }

    #[test]
    fn with_severity_preserves_other_fields() {
        let origin = Location::new(
            Span::new(PathBuf::from("other.rs"), 5, 15),
            "defined here".into(),
        );

        let diag = test_diagnostic()
            .with_origin(origin)
            .with_severity(Severity::Error);

        assert_eq!(diag.rule_id(), RuleId::new("lint.test"));
        assert_eq!(diag.message(), "test message");
        assert_eq!(diag.span().start(), 0);
        assert_eq!(diag.origins().len(), 1);
    }

    #[test]
    fn with_severity_replaces_severity() {
        let diag = test_diagnostic().with_severity(Severity::Error);

        assert_eq!(diag.severity(), Severity::Error);
    }

    #[test]
    fn with_suggestion_adds_suggestion() {
        let suggestion = Suggestion::new(
            Span::new(PathBuf::from("test.rs"), 0, 10),
            "replacement".into(),
            "try this".into(),
        );
        let diag = test_diagnostic().with_suggestion(suggestion);

        assert_eq!(diag.suggestions().len(), 1);
        assert_eq!(diag.suggestions()[0].replacement(), "replacement");
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        fn arb_severity() -> impl Strategy<Value = Severity> {
            prop_oneof![
                Just(Severity::Help),
                Just(Severity::Info),
                Just(Severity::Warn),
                Just(Severity::Error),
            ]
        }

        fn arb_span() -> impl Strategy<Value = Span> {
            (0..=1000usize, 0..=1000usize).prop_map(|(start, delta)| {
                Span::new(PathBuf::from("test.rs"), start, start + delta)
            })
        }

        proptest! {
            #[test]
            fn new_roundtrips_fields(
                severity in arb_severity(),
                message in "\\PC{0,50}",
                span in arb_span(),
            ) {
                let diag = Diagnostic::new(
                    RuleId::new("lint.prop"),
                    severity,
                    message.clone(),
                    span.clone(),
                );

                prop_assert_eq!(diag.rule_id(), RuleId::new("lint.prop"));
                prop_assert_eq!(diag.severity(), severity);
                prop_assert_eq!(diag.message(), message);
                prop_assert_eq!(diag.span(), &span);
            }

            #[test]
            fn new_starts_with_empty_collections(
                severity in arb_severity(),
            ) {
                let diag = Diagnostic::new(
                    RuleId::new("lint.prop"),
                    severity,
                    "msg".into(),
                    Span::new(PathBuf::from("f.rs"), 0, 1),
                );

                prop_assert!(diag.origins().is_empty());
                prop_assert!(diag.related().is_empty());
                prop_assert!(diag.suggestions().is_empty());
            }

            #[test]
            fn with_origin_accumulates(count in 1..=10usize) {
                let mut diag = test_diagnostic();
                for i in 0..count {
                    let origin = Location::new(
                        Span::new(PathBuf::from("f.rs"), i, i + 1),
                        format!("origin {i}"),
                    );
                    diag = diag.with_origin(origin);
                }
                prop_assert_eq!(diag.origins().len(), count);
            }

            #[test]
            fn with_related_accumulates(count in 1..=10usize) {
                let mut diag = test_diagnostic();
                for i in 0..count {
                    let related = Location::new(
                        Span::new(PathBuf::from("f.rs"), i, i + 1),
                        format!("related {i}"),
                    );
                    diag = diag.with_related(related);
                }
                prop_assert_eq!(diag.related().len(), count);
            }

            #[test]
            fn with_suggestion_accumulates(count in 1..=10usize) {
                let mut diag = test_diagnostic();
                for i in 0..count {
                    let suggestion = Suggestion::new(
                        Span::new(PathBuf::from("f.rs"), i, i + 1),
                        format!("fix {i}"),
                        format!("suggestion {i}"),
                    );
                    diag = diag.with_suggestion(suggestion);
                }
                prop_assert_eq!(diag.suggestions().len(), count);
            }
        }
    }
}
