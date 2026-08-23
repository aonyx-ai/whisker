use whisker_rust::RustLintPass;
use whisker_rust::decorations::{FnSignature, ResolvedType, TypePathRef};
use whisker_types::{DecoratedNode, Diagnostic, LintPass, RuleId, Severity};

/// The rule id this probe reports under
const RULE_ID: RuleId = RuleId::new("fixture.anyhow-bare-try");

/// The definition path of `anyhow::Error`
///
/// `anyhow` declares `Error` at its crate root, so the module segment list
/// is empty. The comparison uses the definition path, which is the point
/// of the probe: four crates in the fixture render their error type as
/// `Error`, and only one of them is this one.
const ANYHOW_ERROR: TypePathRef<'static> = TypePathRef::new("anyhow", &[], "Error");

/// Flags `?` on a `Result` inside a function whose error type is anyhow's
///
/// The probe reads three things the provider records and nothing else: the
/// operand's [`ResolvedType`], the enclosing function's [`FnSignature`],
/// and, through the ancestor walk, which body a `?` belongs to. A
/// signature the provider fails to resolve, or resolves to the wrong
/// error type, changes what this reports.
pub struct AnyhowBareTry;

impl AnyhowBareTry {
    /// Creates a boxed [`LintPass`] suitable for the whisker pipeline
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pass = AnyhowBareTry::into_lint_pass();
    /// ```
    pub fn into_lint_pass() -> Box<dyn LintPass> {
        Box::new(whisker_rust::RustLintPassAdapter::new(Self))
    }
}

impl RustLintPass for AnyhowBareTry {
    fn check_try_expression(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let Some(operand) = node.named_child(0) else {
            return Vec::new();
        };

        let Some(resolved) = operand.decoration::<ResolvedType>() else {
            return Vec::new();
        };

        if resolved.is_option() {
            return Vec::new();
        }

        if !resolved.is_result() {
            return Vec::new();
        }

        let Some(signature) = enclosing_fn_signature(node) else {
            return Vec::new();
        };
        let Some(error_type) = signature.error_type() else {
            return Vec::new();
        };
        if !error_type.is(ANYHOW_ERROR) {
            return Vec::new();
        }

        if is_context_call(&operand) {
            return Vec::new();
        }

        vec![Diagnostic::new(
            RULE_ID,
            Severity::Warn,
            "use of `?` on Result without error context".into(),
            node.span(),
        )]
    }
}

/// Node kinds whose bodies own a `?`, so the ancestor walk stops at them
///
/// A closure, an `async` block, a `gen` block, a `const` block, and a
/// `try` block each have their own return type. A `?` inside one converts
/// into that type, not into the enclosing function's error type.
const BODY_BARRIERS: &[&str] = &[
    "async_block",
    "closure_expression",
    "const_block",
    "gen_block",
    "try_block",
];

/// Returns the signature of the function a `?` reports its error to
///
/// The walk stops at a [`BODY_BARRIERS`] kind, so a `?` inside a closure
/// is attributed to the closure rather than to the function around it.
/// The fixture contains exactly that shape, and a barrier that stopped
/// working would show up as an extra flagged expression.
fn enclosing_fn_signature<'a>(node: &DecoratedNode<'a>) -> Option<&'a FnSignature> {
    let mut current = node.parent();

    while let Some(ancestor) = current {
        if BODY_BARRIERS.contains(&ancestor.kind()) {
            return None;
        }
        if ancestor.kind() == "function_item" {
            return ancestor.decoration::<FnSignature>();
        }
        current = ancestor.parent();
    }

    None
}

/// Returns whether the operand is a `.context()` or `.with_context()` call
fn is_context_call(operand: &DecoratedNode<'_>) -> bool {
    if operand.kind() != "call_expression" {
        return false;
    }

    let Some(function) = operand.child_by_field_name("function") else {
        return false;
    };
    if function.kind() != "field_expression" {
        return false;
    }

    let Some(field) = function.child_by_field_name("field") else {
        return false;
    };
    let name = field.text();

    name == "context" || name == "with_context"
}

#[cfg(test)]
mod tests {
    use whisker_rust::decorations::{ErrorType, ReturnMode, TypePath};
    use whisker_testing::{assert_no_diagnostics, decorate, execute, parse};
    use whisker_types::{DecorationMap, Language};

    use super::*;

    /// Returns the id of the first node of `kind` in `tree`
    fn find_id(tree: &whisker_types::DecoratedTree, kind: &str) -> usize {
        fn walk(node: &DecoratedNode<'_>, kind: &str) -> Option<usize> {
            if node.kind() == kind {
                return Some(node.id());
            }
            for child in node.named_children() {
                if let Some(id) = walk(&child, kind) {
                    return Some(id);
                }
            }
            None
        }

        walk(&tree.root_node(), kind).unwrap_or_else(|| panic!("source should hold a {kind}"))
    }

    /// Returns the id of the operand of the first `?` in `tree`
    fn find_try_operand_id(tree: &whisker_types::DecoratedTree) -> usize {
        fn walk(node: &DecoratedNode<'_>) -> Option<usize> {
            if node.kind() == "try_expression" {
                return node.named_child(0).map(|operand| operand.id());
            }
            for child in node.named_children() {
                if let Some(id) = walk(&child) {
                    return Some(id);
                }
            }
            None
        }

        walk(&tree.root_node()).expect("source should hold a try expression")
    }

    /// Runs the probe over `source` with the decorations a provider makes
    ///
    /// These tests attach decorations by hand, which is the opposite of
    /// what the probe exists for. They are here so that a probe broken by
    /// an edit fails in its own package rather than only inside
    /// whisker-rust's provider suite, where the cause would be harder to
    /// see.
    fn run(source: &str, error: ErrorType) -> Vec<Diagnostic> {
        let mut tree = parse(source, Language::Rust);
        let operand = find_try_operand_id(&tree);
        let function = find_id(&tree, "function_item");
        let mut map = DecorationMap::new();
        map.insert(
            operand,
            ResolvedType::new("Result<(), Error>".into()).with_result(true),
        );
        map.insert(
            function,
            FnSignature::new(None, Some(error), ReturnMode::Direct),
        );
        decorate(&mut tree, map);

        execute(&tree, &mut vec![AnyhowBareTry::into_lint_pass()])
    }

    fn anyhow_error() -> ErrorType {
        ErrorType::Named(TypePath::new("anyhow", Vec::<String>::new(), "Error"))
    }

    fn io_error() -> ErrorType {
        ErrorType::Named(TypePath::new("std", vec!["io".to_owned()], "Error"))
    }

    #[test]
    fn bare_try_in_an_anyhow_function_is_flagged() {
        let diagnostics = run("fn f() -> Result<()> { read()?; Ok(()) }", anyhow_error());

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id().as_str(), "fixture.anyhow-bare-try");
    }

    #[test]
    fn bare_try_in_an_io_function_is_not_flagged() {
        let diagnostics = run("fn f() -> Result<()> { read()?; Ok(()) }", io_error());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn try_after_context_is_not_flagged() {
        let diagnostics = run(
            "fn f() -> Result<()> { read().context(\"why\")?; Ok(()) }",
            anyhow_error(),
        );

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AnyhowBareTry>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AnyhowBareTry>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AnyhowBareTry>();
    }
}
