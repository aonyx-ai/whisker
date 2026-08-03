use whisker_rust::RustLintPass;
use whisker_types::{DecoratedNode, Diagnostic, RuleId, Severity};

/// Flags uses of the `matches!` macro
///
/// The `matches!` macro hides the full match expression, making it easy to
/// miss unhandled variants when an enum gains new members. A full `match`
/// expression forces you to consider each variant deliberately.
pub struct NoMatchesMacro;

impl RustLintPass for NoMatchesMacro {
    fn check_macro_invocation(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let Some(macro_node) = node.child_by_field_name("macro") else {
            return Vec::new();
        };
        if macro_node.kind() != "identifier" {
            return Vec::new();
        }
        if macro_node.text() != "matches" {
            return Vec::new();
        }

        vec![Diagnostic::new(
            RuleId("lint.no-matches-macro"),
            Severity::Warn,
            "use a full `match` expression instead of `matches!`".into(),
            node.span(),
        )]
    }
}

#[cfg(test)]
mod tests {
    use whisker_rust::RustLintPassAdapter;
    use whisker_testing::{assert_diagnostic, assert_no_diagnostics, execute, parse};
    use whisker_types::{Language, LintPass, Severity};

    use super::*;

    fn passes() -> Vec<Box<dyn LintPass>> {
        vec![Box::new(RustLintPassAdapter::new(NoMatchesMacro))]
    }

    #[test]
    fn assert_is_not_flagged() {
        let tree = parse("fn f() { assert!(true); }", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn matches_in_function_argument_is_flagged() {
        let source = "fn f() { foo(matches!(x, 1)); }";
        let tree = parse(source, Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn matches_in_let_binding_is_flagged() {
        let source = "fn f() { let _ = matches!(x, Some(_)); }";
        let tree = parse(source, Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .has_rule_id("lint.no-matches-macro")
            .has_severity(Severity::Warn)
            .message_contains("match");
    }

    #[test]
    fn matches_in_return_position_is_flagged() {
        let source = "fn f() -> bool { matches!(x, Some(_)) }";
        let tree = parse(source, Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn matches_with_multiple_patterns_is_flagged() {
        let source = "fn f() { let _ = matches!(x, Foo | Bar); }";
        let tree = parse(source, Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn println_is_not_flagged() {
        let tree = parse("fn f() { println!(\"hello\"); }", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn regular_match_expression_is_not_flagged() {
        let source = "fn f() { match x { 0 => {} _ => {} } }";
        let tree = parse(source, Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<NoMatchesMacro>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<NoMatchesMacro>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<NoMatchesMacro>();
    }
}
