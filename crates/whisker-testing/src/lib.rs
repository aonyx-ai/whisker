use std::path::{Path, PathBuf};

use whisker_types::{DecoratedTree, DecorationMap, Diagnostic, Language, LintPass, Severity, Span};

/// Parses source text into a decorated tree for the given language
///
/// The returned tree has no decorations attached. Use [`decorate`] to
/// add manually constructed decorations for testing.
///
/// # Panics
///
/// Panics if the language is unsupported or if tree-sitter fails to
/// parse.
pub fn parse(source: &str, language: Language) -> DecoratedTree {
    let ts_language = match language {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
    };

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&ts_language)
        .expect("failed to set tree-sitter language");

    let tree = parser
        .parse(source, None)
        .expect("tree-sitter parse returned None");

    DecoratedTree::new(tree, source.to_string(), PathBuf::from("<test>"))
}

/// Attaches manual decorations to a parsed tree
///
/// Replaces the tree's decoration map with the provided one. This
/// allows testing rules that depend on semantic information without
/// running a real language toolchain.
pub fn decorate(tree: &mut DecoratedTree, decorations: DecorationMap) {
    *tree.decorations_mut() = decorations;
}

/// Executes lint passes against a decorated tree and returns diagnostics
pub fn execute(tree: &DecoratedTree, passes: &mut [Box<dyn LintPass>]) -> Vec<Diagnostic> {
    whisker_core::walk(tree, passes)
}

/// Builder for asserting diagnostic properties
///
/// # Examples
///
/// ```ignore
/// let diag = &diagnostics[0];
/// assert_diagnostic(diag)
///     .has_rule_id("lint.test")
///     .has_severity(Severity::Warn)
///     .message_contains("wildcard");
/// ```
pub fn assert_diagnostic(diagnostic: &Diagnostic) -> DiagnosticAssertion<'_> {
    DiagnosticAssertion { diagnostic }
}

/// Asserts that no diagnostics were emitted
///
/// # Panics
///
/// Panics if `diagnostics` is not empty, printing all diagnostics.
pub fn assert_no_diagnostics(diagnostics: &[Diagnostic]) {
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {}: {:?}",
        diagnostics.len(),
        diagnostics
    );
}

/// Loads test fixtures from a directory for the given language
///
/// Each source file matching the language's extension becomes a test
/// case. Returns pairs of `(filename, source_text)` sorted by filename.
///
/// # Panics
///
/// Panics if the directory cannot be read.
pub fn fixtures(dir: &str, language: Language) -> Vec<(String, String)> {
    let path = Path::new(dir);
    let mut cases = Vec::new();

    let entries = std::fs::read_dir(path).unwrap_or_else(|e| panic!("read fixture dir {dir}: {e}"));

    for entry in entries.flatten() {
        let file_path = entry.path();
        let Some(ext) = file_path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if Language::from_extension(ext) != Some(language) {
            continue;
        }
        let name = file_path.file_name().unwrap().to_string_lossy().to_string();
        let source = std::fs::read_to_string(&file_path)
            .unwrap_or_else(|e| panic!("read fixture {}: {e}", file_path.display()));
        cases.push((name, source));
    }

    cases.sort_by(|a, b| a.0.cmp(&b.0));
    cases
}

/// Fluent assertion helper for [`Diagnostic`] values
pub struct DiagnosticAssertion<'a> {
    diagnostic: &'a Diagnostic,
}

impl DiagnosticAssertion<'_> {
    /// Asserts the diagnostic has the given rule ID
    ///
    /// # Panics
    ///
    /// Panics if the rule ID does not match `expected`.
    pub fn has_rule_id(self, expected: &str) -> Self {
        assert_eq!(
            self.diagnostic.rule_id().as_str(),
            expected,
            "expected rule_id `{expected}`, got `{}`",
            self.diagnostic.rule_id()
        );
        self
    }

    /// Asserts the diagnostic has the given severity
    ///
    /// # Panics
    ///
    /// Panics if the severity does not match `expected`.
    pub fn has_severity(self, expected: Severity) -> Self {
        assert_eq!(
            self.diagnostic.severity(),
            expected,
            "expected severity {expected:?}, got {:?}",
            self.diagnostic.severity()
        );
        self
    }

    /// Asserts the diagnostic message contains the given substring
    ///
    /// # Panics
    ///
    /// Panics if the message does not contain `substring`.
    pub fn message_contains(self, substring: &str) -> Self {
        assert!(
            self.diagnostic.message().contains(substring),
            "expected message containing `{substring}`, got `{}`",
            self.diagnostic.message()
        );
        self
    }

    /// Asserts the diagnostic span covers the given byte range
    ///
    /// # Panics
    ///
    /// Panics if the span does not match the expected file, start, and end.
    pub fn has_span(self, file: &str, start: usize, end: usize) -> Self {
        let expected = Span::new(PathBuf::from(file), start, end);
        assert_eq!(
            self.diagnostic.span(),
            &expected,
            "expected span {start}..{end} in {file}, got {}..{} in {}",
            self.diagnostic.span().start(),
            self.diagnostic.span().end(),
            self.diagnostic.span().file().display()
        );
        self
    }

    /// Asserts the diagnostic has the given number of origin locations
    ///
    /// # Panics
    ///
    /// Panics if the origin count does not match `expected`.
    pub fn has_origin_count(self, expected: usize) -> Self {
        assert_eq!(
            self.diagnostic.origins().len(),
            expected,
            "expected {expected} origins, got {}",
            self.diagnostic.origins().len()
        );
        self
    }

    /// Asserts the diagnostic has the given number of related locations
    ///
    /// # Panics
    ///
    /// Panics if the related location count does not match `expected`.
    pub fn has_related_count(self, expected: usize) -> Self {
        assert_eq!(
            self.diagnostic.related().len(),
            expected,
            "expected {expected} related locations, got {}",
            self.diagnostic.related().len()
        );
        self
    }

    /// Asserts the diagnostic has the given number of suggestions
    ///
    /// # Panics
    ///
    /// Panics if the suggestion count does not match `expected`.
    pub fn has_suggestion_count(self, expected: usize) -> Self {
        assert_eq!(
            self.diagnostic.suggestions().len(),
            expected,
            "expected {expected} suggestions, got {}",
            self.diagnostic.suggestions().len()
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use whisker_types::{DecoratedNode, Diagnostic, RuleId, Severity};

    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DiagnosticAssertion<'_>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DiagnosticAssertion<'_>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DiagnosticAssertion<'_>>();
    }

    #[test]
    fn parse_returns_tree_with_source_file_root() {
        let tree = parse("fn main() {}", Language::Rust);
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn execute_with_no_passes_returns_empty() {
        let tree = parse("fn main() {}", Language::Rust);
        let diagnostics = execute(&tree, &mut Vec::new());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn execute_collects_diagnostics_from_pass() {
        struct FnFinder;
        impl LintPass for FnFinder {
            fn check_node(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                if node.kind() == "function_item" {
                    vec![Diagnostic::new(
                        RuleId("test.fn"),
                        Severity::Warn,
                        "found fn".into(),
                        node.span(),
                    )]
                } else {
                    Vec::new()
                }
            }
        }

        let tree = parse("fn main() {}", Language::Rust);
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(FnFinder)];
        let diagnostics = execute(&tree, &mut passes);

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .has_rule_id("test.fn")
            .has_severity(Severity::Warn)
            .message_contains("found fn");
    }

    #[test]
    fn assert_no_diagnostics_with_empty_slice_succeeds() {
        assert_no_diagnostics(&[]);
    }

    #[test]
    #[should_panic(expected = "expected no diagnostics")]
    fn assert_no_diagnostics_with_non_empty_slice_panics() {
        let diag = Diagnostic::new(
            RuleId("test"),
            Severity::Warn,
            "msg".into(),
            whisker_types::Span::new(std::path::PathBuf::from("f.rs"), 0, 1),
        );
        assert_no_diagnostics(&[diag]);
    }

    #[test]
    fn decorate_replaces_decoration_map() {
        let mut tree = parse("fn main() {}", Language::Rust);
        let mut map = DecorationMap::new();
        map.insert(0, 42u32);
        decorate(&mut tree, map);

        assert_eq!(tree.decorations_mut().get::<u32>(0), Some(&42));
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn parse_never_panics(source in "\\PC{0,200}") {
                let _tree = parse(&source, Language::Rust);
            }

            #[test]
            fn parse_root_is_source_file_for_valid_rust(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let tree = parse(&source, Language::Rust);
                let root = tree.root_node();
                prop_assert_eq!(root.kind(), "source_file");
            }

            #[test]
            fn parse_preserves_source(source in "\\PC{0,200}") {
                let tree = parse(&source, Language::Rust);
                prop_assert_eq!(tree.source(), source.as_str());
            }

            #[test]
            fn execute_with_no_passes_always_empty(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let tree = parse(&source, Language::Rust);
                let diagnostics = execute(&tree, &mut Vec::new());
                prop_assert!(diagnostics.is_empty());
            }
        }
    }
}
