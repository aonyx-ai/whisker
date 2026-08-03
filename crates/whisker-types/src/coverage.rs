use crate::{CoverageGap, DecorationMap};

/// The result of offering one file to one decoration provider
///
/// Decorations travel back by value rather than being written through a
/// `&mut DecoratedTree`. That is what makes a declining provider harmless:
/// it never holds a handle it could mutate, so "declined" and "left partial
/// decorations behind" cannot both be true.
///
/// An empty [`DecorationMap`] inside [`Coverage::Covered`] is a legitimate
/// verdict. A file containing only `use` statements has nothing to
/// decorate, and a provider that says so is telling the truth. Coverage is
/// a claim about the *file*, not about individual nodes.
#[must_use]
#[derive(Debug)]
pub enum Coverage {
    /// The provider claims this file, and these are its decorations
    Covered(DecorationMap),

    /// The provider does not claim this file, for this reason
    NotCovered(CoverageGap),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Coverage>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Coverage>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Coverage>();
    }
}
