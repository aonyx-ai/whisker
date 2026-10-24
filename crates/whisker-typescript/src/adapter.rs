use whisker_types::{
    BoxedLintPass, Checked, Configured, DecoratedNode, Language, LintPass, Panic, RuleOptions,
    boxed_lint_pass,
};

use crate::{TypeScriptLintPass, dispatch};

/// Bridges a [`TypeScriptLintPass`] into the platform's [`LintPass`] trait
///
/// This adapter wraps any implementation of the generated
/// `TypeScriptLintPass` trait so it can be passed to the whisker-core
/// pipeline as a `Box<dyn LintPass>`. It names its language as
/// [`Language::TypeScript`], so the walker hands it TypeScript and TSX
/// trees and never a tree of another language. The adapter delegates
/// `check_node` to the generated `dispatch` function, which routes each
/// node to the appropriate typed method based on its kind.
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
/// impl TypeScriptLintPass for MyLint {
///     fn check_function_declaration(&mut self, node: &DecoratedNode) -> Vec<Diagnostic> {
///         // ...
///     }
/// }
///
/// let pass: Box<dyn LintPass> = Box::new(TypeScriptLintPassAdapter::new(MyLint));
/// pipeline.run(path, &providers, &mut vec![pass])?;
/// ```
pub struct TypeScriptLintPassAdapter<P: TypeScriptLintPass> {
    inner: P,
}

impl<P: TypeScriptLintPass> TypeScriptLintPassAdapter<P> {
    /// Wraps a `TypeScriptLintPass` implementation for use with the pipeline
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::{Language, LintPass};
    /// use whisker_typescript::{TypeScriptLintPass, TypeScriptLintPassAdapter};
    ///
    /// struct Quiet;
    /// impl TypeScriptLintPass for Quiet {}
    ///
    /// let pass = TypeScriptLintPassAdapter::new(Quiet);
    ///
    /// assert_eq!(pass.language(), Language::TypeScript);
    /// ```
    pub fn new(pass: P) -> Self {
        Self { inner: pass }
    }

    /// Wraps a rule and boxes it for the plugin boundary
    ///
    /// A factory that whisker-plugin's `export_lints!` writes hands this back
    /// to whisker. It is a pass behind a stabby box and a stabby vtable, which
    /// the host calls without knowing the rule's type.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::{Language, LintPass};
    /// use whisker_typescript::{TypeScriptLintPass, TypeScriptLintPassAdapter};
    ///
    /// struct Quiet;
    /// impl TypeScriptLintPass for Quiet {}
    ///
    /// let pass = TypeScriptLintPassAdapter::boxed(Quiet);
    ///
    /// assert_eq!(pass.language(), Language::TypeScript);
    /// ```
    pub fn boxed(pass: P) -> BoxedLintPass
    where
        P: 'static,
    {
        boxed_lint_pass(Self::new(pass))
    }
}

impl<P: TypeScriptLintPass> LintPass for TypeScriptLintPassAdapter<P> {
    extern "C" fn language(&self) -> Language {
        Language::TypeScript
    }

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

    fn parse_typescript(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&crate::language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(
            tree,
            Language::TypeScript,
            source.to_string(),
            PathBuf::from("test.ts"),
        )
    }

    struct WarnOnFunction;

    impl TypeScriptLintPass for WarnOnFunction {
        fn check_function_declaration(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            vec![Diagnostic::new(
                RuleId::new("test.function"),
                Severity::Warn,
                "found a function".into(),
                node.span(),
            )]
        }
    }

    #[test]
    fn adapter_collects_diagnostics_through_the_walker() {
        let tree = parse_typescript("function a() {}\nfunction b() {}");
        let mut passes: Vec<Box<dyn LintPass>> =
            vec![Box::new(TypeScriptLintPassAdapter::new(WarnOnFunction))];

        let diagnostics = whisker_core::walk(&tree, &mut passes).expect("should walk");

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn adapter_delegates_to_dispatch() {
        let tree = parse_typescript("function main() {}");
        let function = tree.root_node().named_child(0).unwrap();
        let mut adapter = TypeScriptLintPassAdapter::new(WarnOnFunction);

        let checked: Result<_, Panic> = adapter.check_node(&function).into();

        let diagnostics = checked.expect("the rule should not panic");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id(), RuleId::new("test.function"));
    }

    #[test]
    fn adapter_names_typescript_as_its_language() {
        let adapter = TypeScriptLintPassAdapter::new(WarnOnFunction);

        let language = adapter.language();

        assert_eq!(language, Language::TypeScript);
    }

    #[test]
    fn check_node_with_a_panicking_rule_returns_the_panic() {
        struct Exploding;
        impl TypeScriptLintPass for Exploding {
            fn check_function_declaration(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                panic!("cannot check {}", node.kind());
            }
        }

        let tree = parse_typescript("function main() {}");
        let function = tree.root_node().named_child(0).unwrap();
        let mut adapter = TypeScriptLintPassAdapter::new(Exploding);

        let checked: Result<_, Panic> = adapter.check_node(&function).into();

        let panic = checked.expect_err("the rule should panic");
        assert_eq!(panic.message(), "cannot check function_declaration");
    }

    #[test]
    fn configure_with_a_panicking_rule_returns_the_panic() {
        struct Picky;
        impl TypeScriptLintPass for Picky {
            fn configure(&mut self, _options: &RuleOptions) {
                panic!("no options accepted");
            }
        }

        let mut adapter = TypeScriptLintPassAdapter::new(Picky);

        let configured: Result<(), Panic> = adapter.configure(&RuleOptions::default()).into();

        let panic = configured.expect_err("the rule should panic");
        assert_eq!(panic.message(), "no options accepted");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TypeScriptLintPassAdapter<WarnOnFunction>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TypeScriptLintPassAdapter<WarnOnFunction>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TypeScriptLintPassAdapter<WarnOnFunction>>();
    }
}
