use crate::{DecoratedNode, Diagnostic, RuleOptions};

/// A lint rule that inspects decorated syntax tree nodes
///
/// This is the platform-level trait that the tree walker dispatches to.
/// Language-specific SDKs generate more ergonomic traits (e.g.
/// `RustLintPass`) that refine this into per-node-kind methods, but the
/// core pipeline operates on this common interface.
pub trait LintPass: Send + Sync {
    /// Applies the project's options to this pass
    ///
    /// Called once on every pass, after it is constructed and before it
    /// sees a node. A pass the project configured nothing for is still
    /// called, with a table that holds nothing for it.
    ///
    /// The method carries no default, so a language adapter that forwards
    /// nothing does not compile. Configuration that silently does nothing
    /// reads exactly like configuration that works.
    fn configure(&mut self, options: &RuleOptions);

    /// Inspects a single node and returns any diagnostics found
    ///
    /// Called by the tree walker for every named node in the syntax tree.
    /// Implementations should return an empty vec for nodes they do not
    /// care about.
    fn check_node(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;

    impl LintPass for Dummy {
        fn configure(&mut self, _options: &RuleOptions) {}

        fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            Vec::new()
        }
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Dummy>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Dummy>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Dummy>();
    }
}
