use whisker_rust::RustLintPass;
use whisker_rust::decorations::{ErrorType, FnSignature, TypePathRef};
use whisker_types::{DecoratedNode, Diagnostic, RuleId, Severity};

/// The error type this project asks its fallible functions to return
const ANYHOW_ERROR: TypePathRef<'static> = TypePathRef::new("anyhow", &[], "Error");

/// Flags a fallible function whose error type is not `anyhow::Error`
///
/// The rule reads the [`FnSignature`] whisker's decoration provider
/// attaches to every function it resolves, which is the half of the
/// picture a plugin cannot work out for itself. Syntax alone cannot
/// answer the question this rule asks: a return type written `Fallible`
/// hides a `Result` behind an alias, and only the resolved signature sees
/// through it.
///
/// Absence is not a violation. A function the provider never reached
/// carries no signature at all, and a function that cannot fail carries a
/// signature with no error type; both stay quiet, because a rule that
/// guessed would report loudest exactly where it knows least. An error
/// type with no defining item, or one that is still a type parameter,
/// names nothing to compare, so those stay quiet too.
///
/// [`FnSignature`]: whisker_rust::decorations::FnSignature
pub struct AnyhowError;

impl RustLintPass for AnyhowError {
    fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let Some(signature) = node.get::<FnSignature>() else {
            return Vec::new();
        };
        let Some(error_type) = signature.error_type() else {
            return Vec::new();
        };

        let flagged = match error_type {
            ErrorType::Named(_) => !error_type.is(ANYHOW_ERROR),
            ErrorType::Generic => false,
            ErrorType::Unnamed => false,
        };
        if !flagged {
            return Vec::new();
        }

        vec![Diagnostic::new(
            RuleId("custom.anyhow-error"),
            Severity::Warn,
            "return anyhow::Error from a fallible function".into(),
            node.span(),
        )]
    }
}

#[cfg(test)]
mod tests {
    use whisker_rust::RustLintPassAdapter;
    use whisker_rust::decorations::{ResolvedType, ReturnMode, TypePath};
    use whisker_testing::{assert_diagnostic, assert_no_diagnostics, decorate, execute, parse};
    use whisker_types::{DecoratedTree, DecorationMap, Language, LintPass, Severity};

    use super::*;

    /// A function whose return type says nothing about how it can fail
    const SOURCE: &str = "pub fn save() -> Fallible {\n    Ok(())\n}\n";

    fn passes() -> Vec<Box<dyn LintPass>> {
        vec![Box::new(RustLintPassAdapter::new(AnyhowError))]
    }

    /// Parses [`SOURCE`] and attaches `signature` to the function in it
    ///
    /// The decoration provider fills this in during a real run. A unit
    /// test states the signature it wants instead, which is what keeps
    /// these tests independent of a rust-analyzer that has to resolve the
    /// alias first.
    fn tree_with(signature: Option<FnSignature>) -> DecoratedTree {
        let mut tree = parse(SOURCE, Language::Rust);
        let Some(signature) = signature else {
            return tree;
        };

        let function = tree
            .root_node()
            .named_child(0)
            .expect("the source should parse to one function")
            .id();

        let mut decorations = DecorationMap::new();
        decorations.insert(function, signature);
        decorate(&mut tree, decorations);

        tree
    }

    /// A signature that returns a `Result` failing with `error`
    fn fallible(error: Option<ErrorType>) -> FnSignature {
        FnSignature::new(
            Some(ResolvedType::new("Result<(), Error>".into()).with_result(true)),
            error,
            ReturnMode::Direct,
        )
    }

    #[test]
    fn anyhow_error_is_not_flagged() {
        let error = ErrorType::Named(TypePath::new("anyhow", [] as [&str; 0], "Error"));
        let tree = tree_with(Some(fallible(Some(error))));

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn foreign_error_is_flagged() {
        let error = ErrorType::Named(TypePath::new("std", ["io"], "Error"));
        let tree = tree_with(Some(fallible(Some(error))));

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .has_rule_id("custom.anyhow-error")
            .has_severity(Severity::Warn);
    }

    #[test]
    fn function_the_provider_did_not_reach_is_not_flagged() {
        let tree = tree_with(None);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn generic_error_is_not_flagged() {
        let tree = tree_with(Some(fallible(Some(ErrorType::Generic))));

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn infallible_function_is_not_flagged() {
        let tree = tree_with(Some(fallible(None)));

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AnyhowError>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AnyhowError>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AnyhowError>();
    }

    #[test]
    fn unnamed_error_is_not_flagged() {
        let tree = tree_with(Some(fallible(Some(ErrorType::Unnamed))));

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }
}
