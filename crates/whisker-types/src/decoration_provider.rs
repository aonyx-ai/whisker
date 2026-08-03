use crate::{Coverage, DecoratedTree, ProviderName};

/// Attaches semantic decorations to a parsed syntax tree
///
/// Decoration providers bridge between a language toolchain (e.g.
/// rust-analyzer) and the platform's decoration system. A provider reads
/// the syntax tree, queries the toolchain for semantic information, and
/// returns the decorations it produced together with a verdict on whether
/// it had anything to say about the file at all.
///
/// The verdict is not optional and not inferable from the decorations. A
/// provider that returned no decorations because the file has nothing to
/// decorate, and a provider that returned no decorations because it has
/// never heard of the file, are indistinguishable by their output and must
/// be distinguishable by their verdict — otherwise whisker reports a file
/// clean that it could not read the meaning of.
pub trait DecorationProvider: Send + Sync {
    /// Returns the name this provider is identified by in diagnostics
    fn name(&self) -> ProviderName;

    /// Decorates the file, or explains why this provider cannot
    ///
    /// The tree is borrowed immutably and decorations are returned by
    /// value, so a provider that declines has no way to leave partial
    /// state behind and the caller can discard its work without
    /// inspection.
    ///
    /// # Errors
    ///
    /// Returns an error only when the toolchain itself malfunctions — a
    /// poisoned lock, a path that cannot be made absolute. A file the
    /// provider simply has no information about is
    /// [`Coverage::NotCovered`], not an error: returning an error for it
    /// would abort a whole run on the first stray file.
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
