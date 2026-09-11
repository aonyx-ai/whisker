use stabby::slice::Slice;

use crate::Decoration;
use crate::decoration_entry::Entry;

/// A plugin's way into the host's decoration map
///
/// The handle is two borrowed slices: the entries the host holds, and a
/// list of positions into them sorted by node. Both are stabby's, so a
/// plugin reads them at the layout the host wrote, whatever compiler built
/// either side. A lookup is a binary search the plugin runs itself, and it
/// calls into the host only through stabby's checked downcast.
///
/// The handle is [`Copy`] and four words wide, because a [`DecoratedNode`]
/// is [`Copy`] and carries one. It borrows the map, so a decoration it
/// finds lives as long as the map does.
///
/// [`DecoratedNode`]: crate::DecoratedNode
#[stabby::stabby]
#[derive(Copy, Clone)]
pub struct DecorationLookup<'a> {
    order: Slice<'a, usize>,
    entries: Slice<'a, Entry>,
}

impl<'a> DecorationLookup<'a> {
    /// Returns the handle that reads `entries` through `order`
    ///
    /// `order` holds every position in `entries`, sorted by the node each
    /// entry sits on. [`DecorationMap::lookup`] is the only caller.
    ///
    /// [`DecorationMap::lookup`]: crate::DecorationMap
    pub(crate) fn of(order: &'a [usize], entries: &'a [Entry]) -> Self {
        Self {
            order: Slice::from(order),
            entries: Slice::from(entries),
        }
    }

    /// Returns the entries that sit on `node_id`, in insertion order
    fn on(&self, node_id: usize) -> impl Iterator<Item = &'a Entry> {
        let order = self.order.as_slice();
        let entries = self.entries.as_slice();
        let at = |position: &usize| entries[*position].node_id();

        let start = order.partition_point(|position| at(position) < node_id);
        let end = order.partition_point(|position| at(position) <= node_id);

        order[start..end]
            .iter()
            .map(move |&position| &entries[position])
    }

    /// Returns the `index`th decoration of type `T` on the node `node_id`
    ///
    /// The decoration lives as long as the map this handle borrows.
    /// Stabby checks the stored type against `T` before it casts, so a
    /// key that names another type yields [`None`].
    pub(crate) fn get<T: Decoration>(&self, node_id: usize, index: usize) -> Option<&'a T> {
        self.on(node_id)
            .filter(|entry| entry.key() == T::KEY)
            .nth(index)?
            .read::<T>()
    }

    /// Returns every decoration of type `T` on the node `node_id`
    pub(crate) fn get_all<T: Decoration>(&self, node_id: usize) -> Vec<&'a T> {
        self.on(node_id)
            .filter(|entry| entry.key() == T::KEY)
            .filter_map(Entry::read::<T>)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecoratedNode, DecorationKey, DecorationMap};

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Count(u32);

    impl Decoration for Count {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Count"));

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
    fn get_groups_entries_by_node() {
        let mut map = DecorationMap::new();
        map.insert(1, Count(10));
        map.insert(2, Count(20));
        map.insert(1, Count(11));

        let lookup = map.lookup();

        assert_eq!(lookup.get::<Count>(1, 0), Some(&Count(10)));
        assert_eq!(lookup.get::<Count>(1, 1), Some(&Count(11)));
        assert_eq!(lookup.get::<Count>(2, 0), Some(&Count(20)));
        assert!(lookup.get::<Count>(2, 1).is_none());
    }

    #[test]
    fn get_returns_each_entry_under_a_key_in_insertion_order() {
        let mut map = DecorationMap::new();
        map.insert(1, Count(1));
        map.insert(1, Count(2));

        let lookup = map.lookup();

        assert_eq!(lookup.get::<Count>(1, 0), Some(&Count(1)));
        assert_eq!(lookup.get::<Count>(1, 1), Some(&Count(2)));
        assert!(lookup.get::<Count>(1, 2).is_none());
    }

    #[test]
    fn get_with_an_unknown_node_returns_none() {
        let mut map = DecorationMap::new();
        map.insert(1, Count(1));

        let found = map.lookup().get::<Count>(99, 0);

        assert!(found.is_none());
    }

    #[test]
    fn get_with_another_type_returns_none() {
        let mut map = DecorationMap::new();
        map.insert(1, Count(1));

        let found = map.lookup().get::<Other>(1, 0);

        assert!(found.is_none());
    }

    #[test]
    fn get_all_returns_every_entry_of_a_type() {
        let mut map = DecorationMap::new();
        map.insert(1, Count(1));
        map.insert(1, Other(9));
        map.insert(1, Count(2));

        let found = map.lookup().get_all::<Count>(1);

        assert_eq!(found, vec![&Count(1), &Count(2)]);
    }

    #[test]
    fn an_insertion_after_a_lookup_is_visible() {
        let mut map = DecorationMap::new();
        map.insert(1, Count(1));
        assert_eq!(map.lookup().get::<Count>(1, 0), Some(&Count(1)));

        map.insert(1, Count(2));

        assert_eq!(map.lookup().get::<Count>(1, 1), Some(&Count(2)));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecorationLookup<'_>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecorationLookup<'_>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecorationLookup<'_>>();
    }
}
