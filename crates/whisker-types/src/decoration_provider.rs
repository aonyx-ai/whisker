use crate::DecoratedTree;

/// Attaches semantic decorations to a parsed syntax tree
///
/// Decoration providers bridge between a language toolchain (e.g.
/// rust-analyzer) and the platform's decoration system. A provider reads
/// the syntax tree, queries the toolchain for semantic information, and
/// inserts decorations into the tree's [`DecorationMap`].
///
/// [`DecorationMap`]: crate::DecorationMap
pub trait DecorationProvider: Send + Sync {
    /// Populates decorations on the given tree
    ///
    /// # Errors
    ///
    /// Returns an error if the toolchain connection fails or produces
    /// invalid results.
    fn decorate(&self, tree: &mut DecoratedTree) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub;

    impl DecorationProvider for Stub {
        fn decorate(&self, _tree: &mut DecoratedTree) -> anyhow::Result<()> {
            Ok(())
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
