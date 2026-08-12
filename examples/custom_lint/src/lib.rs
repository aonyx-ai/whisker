use whisker_rust::RustLintPass;
use whisker_types::{DecoratedNode, Diagnostic, RuleId, Severity};

/// Flags every `todo!` left in the code
///
/// A `todo!` compiles, so nothing forces it out before a release; a lint
/// keeps each one visible until it is resolved. The rule is syntactic and
/// needs no semantic decorations, which keeps this example small.
///
/// The grammar lets a macro be named by a path, so the rule compares the
/// last segment: `std::todo!` and `core::todo!` count, and so would a
/// `todo!` of your own.
pub struct NoTodo;

impl RustLintPass for NoTodo {
    fn check_macro_invocation(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let Some(macro_node) = node.child_by_field_name("macro") else {
            return Vec::new();
        };
        let name = match macro_node.kind() {
            "identifier" => Some(macro_node),
            "scoped_identifier" => macro_node.child_by_field_name("name"),
            _ => None,
        };
        let Some(name) = name else {
            return Vec::new();
        };
        if name.text() != "todo" {
            return Vec::new();
        }

        vec![Diagnostic::new(
            RuleId("custom.no-todo"),
            Severity::Warn,
            "finish this before it ships".into(),
            node.span(),
        )]
    }
}

whisker_rust::export_lints![NoTodo];

#[cfg(test)]
mod tests {
    use whisker_rust::RustLintPassAdapter;
    use whisker_testing::{assert_diagnostic, assert_no_diagnostics, execute, parse};
    use whisker_types::{Language, LintPass, Severity};

    use super::*;

    fn passes() -> Vec<Box<dyn LintPass>> {
        vec![Box::new(RustLintPassAdapter::new(NoTodo))]
    }

    #[test]
    fn other_macros_are_not_flagged() {
        let tree = parse("fn f() { unimplemented!(); }", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn scoped_todo_is_flagged() {
        let tree = parse("fn f() { std::todo!(); }", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0]).has_rule_id("custom.no-todo");
    }

    #[test]
    fn scoped_other_macro_is_not_flagged() {
        let tree = parse("fn f() { std::unimplemented!(); }", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn todo_is_flagged() {
        let tree = parse("fn f() { todo!(); }", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .has_rule_id("custom.no-todo")
            .has_severity(Severity::Warn);
    }

    #[test]
    fn todo_method_call_is_not_flagged() {
        let tree = parse("fn f(x: Tasks) { x.todo(); }", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<NoTodo>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<NoTodo>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<NoTodo>();
    }
}
