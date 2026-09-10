use stabby::boxed::Box;

use crate::DecorationKey;

/// An owned decoration whose concrete type stabby can check at a cast
///
/// [`Box<dyn Any>`] cannot hold a decoration, because its downcast goes
/// through [`TypeId`], and a plugin's [`TypeId`] for a type differs from
/// the host's. Stabby's [`Any`] answers the same question across images:
/// its vtable carries the type's identity and its layout report, both
/// derived from the definition rather than from the compilation, so a
/// downcast compares what the two images actually agree on.
///
/// [`Any`]: stabby::Any
/// [`Box<dyn Any>`]: std::any::Any
/// [`TypeId`]: std::any::TypeId
pub type Erased = stabby::dynptr!(Box<dyn stabby::Any + Send + Sync>);

/// One decoration, with the node it sits on and the key it went in under
///
/// A plugin reads these out of the host's memory, so stabby lays the entry
/// out. The node id sits in the entry rather than in a separate table, so
/// that a sealed index is one list of positions into one list of entries.
///
/// The type is public because [`DecorationLookup`] names it in the layout
/// stabby records. Its module is not, so nothing outside this crate can
/// name it.
///
/// [`DecorationLookup`]: crate::DecorationLookup
#[stabby::stabby]
pub struct Entry {
    node_id: usize,
    key: DecorationKey,
    value: Erased,
}

impl Entry {
    /// Returns the entry that holds `value` for `node_id` under `key`
    pub(crate) fn new<T: crate::Decoration>(node_id: usize, value: T) -> Self {
        Self {
            node_id,
            key: T::KEY,
            value: Box::new(value).into(),
        }
    }

    /// Returns the node this decoration sits on
    pub(crate) fn node_id(&self) -> usize {
        self.node_id
    }

    /// Returns the key this decoration went in under
    pub(crate) fn key(&self) -> DecorationKey {
        self.key
    }

    /// Returns the decoration as a `T`, or [`None`] if it holds another type
    ///
    /// Stabby compares the stored type's identity and its layout report
    /// against `T`'s before it casts, so a key that names the wrong type
    /// yields [`None`] instead of a misread value.
    pub(crate) fn read<T: crate::Decoration>(&self) -> Option<&T> {
        self.value.stable_downcast_ref::<T, _>()
    }
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("node_id", &self.node_id)
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecoratedNode, Decoration};

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Tag(u32);

    impl Decoration for Tag {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Tag"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Other(u32);

    impl Decoration for Other {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Other"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[test]
    fn read_with_a_mismatched_type_returns_none() {
        let entry = Entry::new(3, Tag(7));

        let read = entry.read::<Other>();

        assert!(read.is_none());
    }

    #[test]
    fn read_with_the_stored_type_returns_the_value() {
        let entry = Entry::new(3, Tag(7));

        let read = entry.read::<Tag>().expect("the entry should hold a Tag");

        assert_eq!(read, &Tag(7));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Entry>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Entry>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Entry>();
    }
}
