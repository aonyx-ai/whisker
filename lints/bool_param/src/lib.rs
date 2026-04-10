use whisker_rust::{RustLintPass, RustLintPassAdapter};
use whisker_types::{DecoratedNode, Diagnostic, LintPass, RuleId, Severity};

const RULE_ID: RuleId = RuleId("lint.bool-param");

/// Flags `bool` parameters in function signatures and `bool` fields in
/// struct definitions
///
/// Boolean parameters and fields obscure intent at call sites and in data
/// models. An enum with meaningful variant names makes the code
/// self-documenting and prevents accidental transposition of arguments.
pub struct BoolParam;

impl BoolParam {
    /// Creates a boxed [`LintPass`] suitable for the whisker pipeline
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pass = BoolParam::into_lint_pass();
    /// ```
    pub fn into_lint_pass() -> Box<dyn LintPass> {
        Box::new(RustLintPassAdapter::new(Self))
    }
}

/// Returns whether the given node is a `primitive_type` with text `"bool"`
fn is_bool_type(node: &DecoratedNode<'_>) -> bool {
    node.kind() == "primitive_type" && node.text() == "bool"
}

impl RustLintPass for BoolParam {
    // r[impl lint.bool-param.detect-fn]
    fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let Some(parameters) = node.child_by_field_name("parameters") else {
            return Vec::new();
        };

        let mut diagnostics = Vec::new();
        for param in parameters.named_children() {
            if param.kind() != "parameter" {
                continue;
            }
            let Some(ty) = param.child_by_field_name("type") else {
                continue;
            };
            if is_bool_type(&ty) {
                // r[impl lint.bool-param.message]
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Warn,
                    "parameter has type `bool`; use an enum with meaningful variants".into(),
                    ty.span(),
                ));
            }
        }
        diagnostics
    }

    // r[impl lint.bool-param.detect-struct]
    fn check_struct_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let Some(body) = node.child_by_field_name("body") else {
            return Vec::new();
        };

        let mut diagnostics = Vec::new();
        for child in body.named_children() {
            let ty = match child.kind() {
                "field_declaration" => child.child_by_field_name("type"),
                _ => None,
            };
            let Some(ty) = ty else {
                continue;
            };
            if is_bool_type(&ty) {
                // r[impl lint.bool-param.message]
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Warn,
                    "struct field has type `bool`; use an enum with meaningful variants".into(),
                    ty.span(),
                ));
            }
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use whisker_testing::{assert_diagnostic, assert_no_diagnostics, execute, parse};
    use whisker_types::{Language, LintPass, Severity};

    use super::*;

    fn adapt() -> Box<dyn LintPass> {
        BoolParam::into_lint_pass()
    }

    fn run(source: &str) -> Vec<Diagnostic> {
        let tree = parse(source, Language::Rust);
        let mut passes = vec![adapt()];
        execute(&tree, &mut passes)
    }

    #[test]
    fn bool_local_variable_not_flagged() {
        let diagnostics = run("fn foo() { let x: bool = true; }");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn bool_param_in_function_flagged() {
        let diagnostics = run("fn foo(x: bool) {}");

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .has_rule_id("lint.bool-param")
            .has_severity(Severity::Warn)
            .message_contains("parameter has type `bool`");
    }

    #[test]
    fn bool_return_type_not_flagged() {
        let diagnostics = run("fn foo() -> bool { true }");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn bool_struct_field_flagged() {
        let diagnostics = run("struct Config { verbose: bool }");

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .has_rule_id("lint.bool-param")
            .has_severity(Severity::Warn)
            .message_contains("struct field has type `bool`");
    }

    #[test]
    fn multiple_bool_params_each_flagged() {
        let diagnostics = run("fn foo(a: bool, b: bool) {}");

        assert_eq!(diagnostics.len(), 2);
        assert_diagnostic(&diagnostics[0])
            .has_rule_id("lint.bool-param")
            .has_severity(Severity::Warn)
            .message_contains("parameter has type `bool`");
        assert_diagnostic(&diagnostics[1])
            .has_rule_id("lint.bool-param")
            .has_severity(Severity::Warn)
            .message_contains("parameter has type `bool`");
    }

    #[test]
    fn non_bool_param_not_flagged() {
        let diagnostics = run("fn foo(x: i32, y: String) {}");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<BoolParam>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<BoolParam>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<BoolParam>();
    }
}
