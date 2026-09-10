use whisker_types::{
    BoxedLintPass, Checked, Configured, DecoratedNode, LintPass, Panic, RuleOptions,
    boxed_lint_pass,
};

use crate::{RustLintPass, dispatch};

/// Bridges a [`RustLintPass`] into the platform's [`LintPass`] trait
///
/// This adapter wraps any implementation of the generated `RustLintPass`
/// trait so it can be passed to the whisker-core pipeline as a
/// `Box<dyn LintPass>`. The adapter delegates `check_node` to the
/// generated `dispatch` function, which routes each node to the
/// appropriate typed method based on its kind.
///
/// The adapter is also the plugin's edge. A rule may panic, and a panic
/// must not unwind into the host. Each call therefore runs under
/// [`Panic::catch`] and comes back as a value. A rule keeps writing plain
/// Rust and returning std's `Vec`; the copy into the list that crosses the
/// boundary happens here.
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

    /// Wraps a rule and boxes it for the plugin boundary
    ///
    /// This is what a factory written by [`export_lints!`] hands back to
    /// whisker: a pass behind a stabby box and a stabby vtable, which the
    /// host calls without knowing the rule's type.
    ///
    /// [`export_lints!`]: crate::export_lints
    pub fn boxed(pass: P) -> BoxedLintPass
    where
        P: 'static,
    {
        boxed_lint_pass(Self::new(pass))
    }
}

impl<P: RustLintPass> LintPass for RustLintPassAdapter<P> {
    extern "C" fn configure(&mut self, options: &RuleOptions) -> Configured {
        Panic::catch(|| self.inner.configure(options)).into()
    }

    extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
        Panic::catch(|| dispatch(&mut self.inner, node))
            .map(|found| found.into_iter().collect())
            .into()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{DecoratedTree, Diagnostic, RuleId, Severity};

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

        let checked: Result<_, Panic> = adapter.check_node(&fn_node).into();

        let diagnostics = checked.expect("the rule should not panic");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id(), RuleId::new("test.fn"));
    }

    #[test]
    fn adapter_works_with_pipeline() {
        struct NoOp;
        impl RustLintPass for NoOp {}

        let tree = parse_rust("fn main() {}");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(RustLintPassAdapter::new(NoOp))];

        let diagnostics = whisker_core::walk(&tree, &mut passes).expect("should walk");
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

        let diagnostics = whisker_core::walk(&tree, &mut passes).expect("should walk");
        assert_eq!(diagnostics.len(), 2);
    }

    /// A rule that panics hands the panic back rather than unwinding
    ///
    /// The adapter is where a plugin's edge is, so this is the one place
    /// that proves a panic becomes a value with the message intact.
    #[test]
    fn check_node_with_a_panicking_rule_returns_the_panic() {
        struct Exploding;
        impl RustLintPass for Exploding {
            fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                panic!("cannot check {}", node.kind());
            }
        }

        let tree = parse_rust("fn main() {}");
        let fn_node = tree.root_node().named_child(0).unwrap();
        let mut adapter = RustLintPassAdapter::new(Exploding);

        let checked: Result<_, Panic> = adapter.check_node(&fn_node).into();

        let panic = checked.expect_err("the rule should panic");
        assert_eq!(panic.message(), "cannot check function_item");
    }

    #[test]
    fn configure_with_a_panicking_rule_returns_the_panic() {
        struct Picky;
        impl RustLintPass for Picky {
            fn configure(&mut self, _options: &RuleOptions) {
                panic!("no options accepted");
            }
        }

        let mut adapter = RustLintPassAdapter::new(Picky);

        let configured: Result<(), Panic> = adapter.configure(&RuleOptions::default()).into();

        let panic = configured.expect_err("the rule should panic");
        assert_eq!(panic.message(), "no options accepted");
    }
}
