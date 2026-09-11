use stabby::{result, vec};

use crate::{DecoratedNode, Diagnostic, Panic, RuleOptions};

/// A lint rule that inspects decorated syntax tree nodes
///
/// This is the platform-level trait that the tree walker dispatches to.
/// Language-specific SDKs generate more ergonomic traits (e.g.
/// `RustLintPass`) that refine this into per-node-kind methods, but the
/// core pipeline operates on this common interface.
///
/// A pass lives in a plugin and the host calls it, so stabby generates the
/// vtable and every method is `extern "C"`. Rust's own calling convention
/// is a promise only one compiler makes, and so is the vtable std would
/// build for a trait object. Each method hands back a result whose error
/// is a [`Panic`]. A plugin catches a panic at its edge, because an unwind
/// must not cross the boundary, and the host reports what it caught.
#[stabby::stabby]
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
    extern "C" fn configure(&mut self, options: &RuleOptions) -> Configured;

    /// Inspects a single node and returns any diagnostics found
    ///
    /// Called by the tree walker for every named node in the syntax tree.
    /// Implementations should return an empty list for nodes they do not
    /// care about.
    extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked;
}

/// What [`LintPass::configure`] hands back: nothing, or the panic that stopped it
pub type Configured = result::Result<(), Panic>;

/// What [`LintPass::check_node`] hands back: diagnostics, or the panic that stopped it
///
/// The list is stabby's, because it crosses the boundary. The plugin
/// allocates it and whisker frees it, which is sound because every
/// allocation stabby makes carries its own release function.
pub type Checked = result::Result<vec::Vec<Diagnostic>, Panic>;

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;

    impl LintPass for Dummy {
        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
            Checked::Ok(vec::Vec::new())
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
