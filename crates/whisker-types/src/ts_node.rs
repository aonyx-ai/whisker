use std::ffi::c_void;

/// The layout of tree-sitter's `TSNode`, for stabby's benefit
///
/// [`DecoratedNode`] holds a `tree_sitter::Node`, which is
/// `#[repr(transparent)]` over `TSNode`, a C struct of four `u32`s and two
/// pointers. Stabby cannot look inside a foreign type, so the node sits in
/// a [`StableLike`] that names this struct as its layout.
/// [`StableLike::new`] refuses to compile if the two differ in size or
/// alignment.
///
/// Nothing constructs one. The `Send` and `Sync` implementations exist
/// because [`StableLike`] carries this type in a `PhantomData`. Without
/// them that `PhantomData` would strip both traits from [`DecoratedNode`],
/// which `tree_sitter::Node` itself has.
///
/// The type is public because stabby names it in the layout it records
/// for [`DecoratedNode`]. Its module is not, so nothing outside this crate
/// can name it.
///
/// [`DecoratedNode`]: crate::DecoratedNode
/// [`StableLike`]: stabby::abi::StableLike
/// [`StableLike::new`]: stabby::abi::StableLike::new
#[stabby::stabby]
pub struct TsNode {
    context: [u32; 4],
    id: *const c_void,
    tree: *const c_void,
}

// SAFETY: nothing constructs a `TsNode`, so no value of it is ever sent
// or shared. The implementations exist so that the `PhantomData` inside
// `StableLike` does not strip `Send` and `Sync` from `DecoratedNode`,
// which `tree_sitter::Node` has.
unsafe impl Send for TsNode {}
// SAFETY: as for `Send` above.
unsafe impl Sync for TsNode {}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, size_of};

    use super::*;

    #[test]
    fn layout_matches_the_tree_sitter_node() {
        assert_eq!(size_of::<TsNode>(), size_of::<tree_sitter::Node<'_>>());
        assert_eq!(align_of::<TsNode>(), align_of::<tree_sitter::Node<'_>>());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TsNode>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TsNode>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TsNode>();
    }
}
