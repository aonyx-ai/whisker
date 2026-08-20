use whisker_types::{DecoratedNode, Diagnostic, LintPass};

use crate::{RustLintPass, dispatch};

/// Bridges a [`RustLintPass`] into the platform's [`LintPass`] trait
///
/// This adapter wraps any implementation of the generated `RustLintPass`
/// trait so it can be passed to the whisker-core pipeline as a
/// `Box<dyn LintPass>`. The adapter delegates `check_node` to the
/// generated `dispatch` function, which routes each node to the
/// appropriate typed method based on its kind.
///
/// # Examples
///
/// ```ignore
/// struct MyLint;
/// impl RustLintPass for MyLint {
///     fn check_function_item(&mut self, node: &DecoratedNode) -> Vec<Diagnostic> {
///         // ...
///     }
/// }
///
/// let pass: Box<dyn LintPass> = Box::new(RustLintPassAdapter::new(MyLint));
/// pipeline.run(path, &providers, &mut vec![pass])?;
/// ```
pub struct RustLintPassAdapter<P: RustLintPass> {
    inner: P,
}

impl<P: RustLintPass> RustLintPassAdapter<P> {
    /// Wraps a `RustLintPass` implementation for use with the pipeline
    pub fn new(pass: P) -> Self {
        Self { inner: pass }
    }
}

impl<P: RustLintPass> LintPass for RustLintPassAdapter<P> {
    fn check_node(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        dispatch(&mut self.inner, node)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{DecoratedTree, RuleId, Severity};

    use super::*;

    fn parse_rust(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&crate::language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(tree, source.to_string(), PathBuf::from("test.rs"))
    }

    #[test]
    fn trait_send() {
        struct Dummy;
        impl RustLintPass for Dummy {}
        fn assert_send<T: Send>() {}
        assert_send::<RustLintPassAdapter<Dummy>>();
    }

    #[test]
    fn trait_sync() {
        struct Dummy;
        impl RustLintPass for Dummy {}
        fn assert_sync<T: Sync>() {}
        assert_sync::<RustLintPassAdapter<Dummy>>();
    }

    #[test]
    fn trait_unpin() {
        struct Dummy;
        impl RustLintPass for Dummy {}
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RustLintPassAdapter<Dummy>>();
    }

    #[test]
    fn adapter_delegates_to_dispatch() {
        struct FnFinder {
            found: bool,
        }
        impl RustLintPass for FnFinder {
            fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                self.found = true;
                vec![Diagnostic::new(
                    RuleId::new("test.fn"),
                    Severity::Warn,
                    "found".into(),
                    node.span(),
                )]
            }
        }

        let tree = parse_rust("fn main() {}");
        let fn_node = tree.root_node().named_child(0).unwrap();
        let mut adapter = RustLintPassAdapter::new(FnFinder { found: false });

        let diagnostics = adapter.check_node(&fn_node);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id(), RuleId::new("test.fn"));
    }

    #[test]
    fn adapter_works_with_pipeline() {
        struct NoOp;
        impl RustLintPass for NoOp {}

        let tree = parse_rust("fn main() {}");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(RustLintPassAdapter::new(NoOp))];

        let diagnostics = whisker_core::walk(&tree, &mut passes);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn adapter_collects_diagnostics_through_pipeline() {
        struct WarnOnFn;
        impl RustLintPass for WarnOnFn {
            fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                vec![Diagnostic::new(
                    RuleId::new("test.warn"),
                    Severity::Warn,
                    "function found".into(),
                    node.span(),
                )]
            }
        }

        let tree = parse_rust("fn a() {} fn b() {}");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(RustLintPassAdapter::new(WarnOnFn))];

        let diagnostics = whisker_core::walk(&tree, &mut passes);
        assert_eq!(diagnostics.len(), 2);
    }
}
