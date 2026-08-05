use crate::{CoverageGap, DecorationMap};

/// The verdict from one decoration provider for one file
///
/// An empty [`DecorationMap`] inside [`Coverage::Covered`] is a valid
/// verdict: coverage is a claim about the file, not about individual
/// nodes. A file with nothing to decorate is still covered.
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
