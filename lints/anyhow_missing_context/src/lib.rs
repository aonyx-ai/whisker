use whisker_rust::RustLintPass;
use whisker_rust::decorations::{FnSignature, ResolvedType, TypePathRef};
use whisker_types::{DecoratedNode, Diagnostic, LintPass, RuleId, Severity};

const RULE_ID: RuleId = RuleId("lint.anyhow-missing-context");

/// The one error type this rule is about
///
/// `anyhow` declares `Error` at its crate root, so its definition path and
/// its public path coincide and the module segment list is empty. A crate
/// with its own `Error` type, or one that does not depend on anyhow at all,
/// can never match this however the type prints.
const ANYHOW_ERROR: TypePathRef<'static> = TypePathRef::new("anyhow", &[], "Error");

/// Flags uses of `?` on [`Result`] types without a preceding `.context()`
/// or `.with_context()` call, but only when the enclosing function returns
/// `Result<T, anyhow::Error>`
///
/// Using `?` without `.context()` propagates errors without adding
/// information about what operation failed. Rich error context makes
/// debugging significantly easier by providing a chain of explanations
/// for how the error occurred.
///
/// A `?` inside a closure, an `async` block, or a `try` block converts into
/// that body's error type rather than the enclosing function's, so it is
/// attributed to the body and left alone. Suggesting `.context(..)` there
/// would propose a fix that does not compile.
///
/// [`Result`]: std::result::Result
pub struct AnyhowMissingContext;

impl AnyhowMissingContext {
    /// Creates a boxed [`LintPass`] suitable for the whisker pipeline
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pass = AnyhowMissingContext::into_lint_pass();
    /// ```
    pub fn into_lint_pass() -> Box<dyn LintPass> {
        Box::new(whisker_rust::RustLintPassAdapter::new(Self))
    }
}

impl RustLintPass for AnyhowMissingContext {
    // r[impl lint.anyhow-missing-context.detect]
    fn check_try_expression(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let Some(operand) = node.named_child(0) else {
            return Vec::new();
        };

        let Some(resolved) = operand.decoration::<ResolvedType>() else {
            return Vec::new();
        };

        // r[impl lint.anyhow-missing-context.option-ignored]
        if resolved.is_option() {
            return Vec::new();
        }

        if !resolved.is_result() {
            return Vec::new();
        }

        // r[impl lint.anyhow-missing-context.anyhow-only]
        let Some(fn_sig) = find_enclosing_fn_signature(node) else {
            return Vec::new();
        };
        let Some(error_type) = fn_sig.error_type() else {
            return Vec::new();
        };
        if !error_type.is(ANYHOW_ERROR) {
            return Vec::new();
        }

        // r[impl lint.anyhow-missing-context.context-allowed]
        // r[impl lint.anyhow-missing-context.with-context-allowed]
        if is_context_call(&operand) {
            return Vec::new();
        }

        // r[impl lint.anyhow-missing-context.message]
        vec![Diagnostic::new(
            RULE_ID,
            Severity::Warn,
            "use of `?` on Result without error context".into(),
            node.span(),
        )]
    }
}

/// Node kinds that own a `?`, so the walk must not climb past them
///
/// A closure, an `async` block, a `gen` block, a `const` block, and a `try`
/// block each have their own return type, and `?` inside one converts into
/// that type rather than the enclosing function's. Attributing it to the
/// function would report an error type the expression never sees — and,
/// worse, would suggest `.context(..)` where it does not compile.
const BODY_BARRIERS: &[&str] = &[
    "async_block",
    "closure_expression",
    "const_block",
    "gen_block",
    "try_block",
];

/// Walks up the tree to find the enclosing `function_item` and returns
/// its [`FnSignature`] decoration, if present
///
/// Returns [`None`] on reaching a body that owns the `?` before reaching a
/// function. That is a false negative by construction — a `?` inside a
/// closure that genuinely returns `anyhow::Result` is no longer flagged —
/// and it is the right direction for a rule that runs in CI.
fn find_enclosing_fn_signature<'a>(node: &DecoratedNode<'a>) -> Option<&'a FnSignature> {
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
    use whisker_rust::decorations::{ErrorType, FnSignature, ResolvedType, ReturnMode, TypePath};
    use whisker_testing::{assert_diagnostic, assert_no_diagnostics, decorate, execute, parse};
    use whisker_types::{DecorationMap, Language, LintPass, Severity};

    use super::*;

    fn passes() -> Vec<Box<dyn LintPass>> {
        vec![AnyhowMissingContext::into_lint_pass()]
    }

    fn result_type() -> ResolvedType {
        ResolvedType::new("Result<(), anyhow::Error>".into()).with_result(true)
    }

    fn option_type() -> ResolvedType {
        ResolvedType::new("Option<String>".into()).with_option(true)
    }

    fn anyhow_fn_sig() -> FnSignature {
        let ret = ResolvedType::new("Result<(), anyhow::Error>".into()).with_result(true);
        let error = ErrorType::Named(TypePath::new("anyhow", [] as [&str; 0], "Error"));
        FnSignature::new(Some(ret), Some(error), ReturnMode::Direct)
    }

    fn non_anyhow_fn_sig() -> FnSignature {
        let ret = ResolvedType::new("Result<(), std::io::Error>".into()).with_result(true);
        let error = ErrorType::Named(TypePath::new("core", ["io", "error"], "Error"));
        FnSignature::new(Some(ret), Some(error), ReturnMode::Direct)
    }

    /// Walks the tree to find the first node with the given kind
    fn find_node_by_kind<'a>(node: &whisker_types::DecoratedNode<'a>, kind: &str) -> Option<usize> {
        if node.kind() == kind {
            return Some(node.id());
        }
        for child in node.named_children() {
            if let Some(id) = find_node_by_kind(&child, kind) {
                return Some(id);
            }
        }
        None
    }

    /// Walks the tree to find the first `try_expression` and returns the
    /// node ID of its operand (first named child)
    fn find_try_operand_id(tree: &whisker_types::DecoratedTree) -> usize {
        fn walk(node: &whisker_types::DecoratedNode<'_>) -> Option<usize> {
            if node.kind() == "try_expression" {
                return node.named_child(0).map(|c| c.id());
            }
            for child in node.named_children() {
                if let Some(id) = walk(&child) {
                    return Some(id);
                }
            }
            None
        }
        walk(&tree.root_node()).expect("should find try_expression operand")
    }

    #[test]
    fn in_anyhow_fn_without_context_is_flagged() {
        let source = "fn foo() -> Result<(), Error> { something()?; }";
        let mut tree = parse(source, Language::Rust);
        let operand_id = find_try_operand_id(&tree);
        let fn_id = find_node_by_kind(&tree.root_node(), "function_item").expect("should find fn");

        let mut map = DecorationMap::new();
        map.insert(operand_id, result_type());
        map.insert(fn_id, anyhow_fn_sig());
        decorate(&mut tree, map);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .has_rule_id("lint.anyhow-missing-context")
            .has_severity(Severity::Warn)
            .message_contains("without error context");
    }

    /// A `?` inside an `async` block converts into that block's error type
    ///
    /// The block is its own body, so the enclosing `fn`'s signature says
    /// nothing about what the `?` there produces.
    #[test]
    fn in_async_block_inside_anyhow_fn_is_not_flagged() {
        let source = "fn foo() -> Result<(), Error> { async { something()?; } }";
        let mut tree = parse(source, Language::Rust);
        let operand_id = find_try_operand_id(&tree);
        let fn_id = find_node_by_kind(&tree.root_node(), "function_item").expect("should find fn");

        let mut map = DecorationMap::new();
        map.insert(operand_id, result_type());
        map.insert(fn_id, anyhow_fn_sig());
        decorate(&mut tree, map);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    /// A `?` inside a closure converts into the closure's error type
    ///
    /// Reporting the enclosing function's error type here would suggest
    /// `.context(..)` inside a closure returning something else, which does
    /// not compile.
    #[test]
    fn in_closure_inside_anyhow_fn_is_not_flagged() {
        let source = "fn foo() -> Result<(), Error> { let f = || { something()?; }; }";
        let mut tree = parse(source, Language::Rust);
        let operand_id = find_try_operand_id(&tree);
        let fn_id = find_node_by_kind(&tree.root_node(), "function_item").expect("should find fn");

        let mut map = DecorationMap::new();
        map.insert(operand_id, result_type());
        map.insert(fn_id, anyhow_fn_sig());
        decorate(&mut tree, map);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn in_non_anyhow_fn_is_not_flagged() {
        let source = "fn foo() -> Result<(), IoError> { something()?; }";
        let mut tree = parse(source, Language::Rust);
        let operand_id = find_try_operand_id(&tree);
        let fn_id = find_node_by_kind(&tree.root_node(), "function_item").expect("should find fn");

        let mut map = DecorationMap::new();
        map.insert(operand_id, result_type());
        map.insert(fn_id, non_anyhow_fn_sig());
        decorate(&mut tree, map);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn no_resolved_type_decoration_is_not_flagged() {
        let source = "fn foo() -> Result<(), Error> { something()?; }";
        let mut tree = parse(source, Language::Rust);
        let fn_id = find_node_by_kind(&tree.root_node(), "function_item").expect("should find fn");

        let mut map = DecorationMap::new();
        map.insert(fn_id, anyhow_fn_sig());
        decorate(&mut tree, map);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn on_option_is_not_flagged() {
        let source = "fn foo() -> Result<(), Error> { something()?; }";
        let mut tree = parse(source, Language::Rust);
        let operand_id = find_try_operand_id(&tree);
        let fn_id = find_node_by_kind(&tree.root_node(), "function_item").expect("should find fn");

        let mut map = DecorationMap::new();
        map.insert(operand_id, option_type());
        map.insert(fn_id, anyhow_fn_sig());
        decorate(&mut tree, map);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AnyhowMissingContext>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AnyhowMissingContext>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AnyhowMissingContext>();
    }

    #[test]
    fn with_context_call_is_not_flagged() {
        let source = "fn foo() -> Result<(), Error> { something().with_context(|| \"msg\")?; }";
        let mut tree = parse(source, Language::Rust);
        let operand_id = find_try_operand_id(&tree);
        let fn_id = find_node_by_kind(&tree.root_node(), "function_item").expect("should find fn");

        let mut map = DecorationMap::new();
        map.insert(operand_id, result_type());
        map.insert(fn_id, anyhow_fn_sig());
        decorate(&mut tree, map);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn with_dot_context_call_is_not_flagged() {
        let source = "fn foo() -> Result<(), Error> { something().context(\"reading file\")?; }";
        let mut tree = parse(source, Language::Rust);
        let operand_id = find_try_operand_id(&tree);
        let fn_id = find_node_by_kind(&tree.root_node(), "function_item").expect("should find fn");

        let mut map = DecorationMap::new();
        map.insert(operand_id, result_type());
        map.insert(fn_id, anyhow_fn_sig());
        decorate(&mut tree, map);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }
}
