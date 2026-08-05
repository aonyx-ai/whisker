use crate::{Coverage, DecoratedTree, ProviderName};

/// Attaches semantic decorations to a parsed syntax tree
///
/// Decoration providers bridge between a language toolchain (e.g.
/// rust-analyzer) and the platform's decoration system. A provider reads
/// the syntax tree, queries the toolchain, and returns decorations
/// together with a [`Coverage`] verdict.
///
/// The verdict tells the caller whether the provider could analyze the
/// file. An empty decoration set is ambiguous: a file with nothing to
/// decorate and an unknown file produce the same output.
pub trait DecorationProvider: Send + Sync {
    /// Returns the name that identifies this provider in diagnostics
    fn name(&self) -> ProviderName;

    /// Decorates the file, or explains why the provider declines it
    ///
    /// The provider cannot mutate the tree, so a declined file carries no
    /// partial state.
    ///
    /// # Errors
    ///
    /// Returns an error only when the toolchain itself malfunctions, for
    /// example a poisoned lock. Lack of information about a file is not
    /// an error; the provider reports [`Coverage::NotCovered`] instead.
    fn decorate(&self, tree: &DecoratedTree) -> anyhow::Result<Coverage>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecorationMap;

    struct Stub;

    impl DecorationProvider for Stub {
        fn name(&self) -> ProviderName {
            ProviderName("stub")
        }

        fn decorate(&self, _tree: &DecoratedTree) -> anyhow::Result<Coverage> {
            Ok(Coverage::Covered(DecorationMap::new()))
        }
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Stub>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Stub>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Stub>();
    }
}
