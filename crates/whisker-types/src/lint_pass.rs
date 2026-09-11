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

/// A pass boxed for the boundary: a stabby box with a stabby vtable
///
/// A plugin builds one from a pass it owns, and the host calls through it
/// without knowing the pass's type. The host can also treat it as a
/// [`LintPass`] of its own, because this type implements the trait by
/// forwarding through the vtable. The rest of whisker therefore keeps its
/// `Box<dyn LintPass>`.
pub type BoxedLintPass = stabby::dynptr!(stabby::boxed::Box<dyn LintPass + Send + Sync + 'static>);

/// Boxes a pass for the boundary
///
/// # Examples
///
/// ```
/// use stabby::vec;
/// use whisker_types::{
///     Checked, Configured, DecoratedNode, LintPass, RuleOptions, boxed_lint_pass,
/// };
///
/// struct Quiet;
///
/// impl LintPass for Quiet {
///     extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
///         Configured::Ok(())
///     }
///
///     extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
///         Checked::Ok(vec::Vec::new())
///     }
/// }
///
/// let mut pass = boxed_lint_pass(Quiet);
///
/// let configured: Result<(), _> = pass.configure(&RuleOptions::default()).into();
/// assert!(configured.is_ok());
/// ```
pub fn boxed_lint_pass(pass: impl LintPass + 'static) -> BoxedLintPass {
    stabby::boxed::Box::new(pass).into()
}

impl LintPass for BoxedLintPass {
    extern "C" fn configure(&mut self, options: &RuleOptions) -> Configured {
        LintPassDynMut::configure(self, options)
    }

    extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
        LintPassDynMut::check_node(self, node)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

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
    fn a_boxed_pass_forwards_through_its_vtable() {
        static CONFIGURED: AtomicU32 = AtomicU32::new(0);
        struct Counting;

        impl LintPass for Counting {
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                CONFIGURED.fetch_add(1, Ordering::SeqCst);
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                Checked::Ok(vec::Vec::new())
            }
        }

        let mut pass: Box<dyn LintPass> = Box::new(boxed_lint_pass(Counting));

        let configured: Result<(), Panic> = pass.configure(&RuleOptions::default()).into();

        assert!(configured.is_ok());
        assert_eq!(CONFIGURED.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Dummy>();
        assert_send::<BoxedLintPass>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Dummy>();
        assert_sync::<BoxedLintPass>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Dummy>();
        assert_unpin::<BoxedLintPass>();
    }
}
