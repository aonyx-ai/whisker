use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Storage for per-node decorations on a syntax tree
///
/// Decorations are semantic annotations attached to tree-sitter nodes by
/// decoration providers. Each node (keyed by its `id()`) can carry multiple
/// decorations of different types. Decorations are type-erased via
/// [`Any`] and retrieved by downcast.
///
/// [`Any`]: std::any::Any
#[derive(Default, Debug)]
pub struct DecorationMap {
    entries: HashMap<usize, Vec<ErasedDecoration>>,
}

#[derive(Debug)]
struct ErasedDecoration {
    type_id: TypeId,
    value: Box<dyn Any + Send + Sync>,
}

impl DecorationMap {
    /// Creates an empty decoration map
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches a decoration to the node with the given tree-sitter node ID
    pub fn insert<T: Any + Send + Sync>(&mut self, node_id: usize, value: T) {
        let entry = ErasedDecoration {
            type_id: TypeId::of::<T>(),
            value: Box::new(value),
        };
        self.entries.entry(node_id).or_default().push(entry);
    }

    /// Retrieves the first decoration of type `T` attached to the given node
    ///
    /// Returns [`None`] if no decoration of that type exists for this node.
    pub fn get<T: Any + Send + Sync>(&self, node_id: usize) -> Option<&T> {
        let decorations = self.entries.get(&node_id)?;
        let target = TypeId::of::<T>();
        decorations
            .iter()
            .find(|d| d.type_id == target)
            .and_then(|d| d.value.downcast_ref())
    }

    /// Retrieves all decorations of type `T` attached to the given node
    pub fn get_all<T: Any + Send + Sync>(&self, node_id: usize) -> Vec<&T> {
        let Some(decorations) = self.entries.get(&node_id) else {
            return Vec::new();
        };
        let target = TypeId::of::<T>();
        decorations
            .iter()
            .filter(|d| d.type_id == target)
            .filter_map(|d| d.value.downcast_ref())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TypeInfo(String);

    #[derive(Debug)]
    struct Scope(u32);

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecorationMap>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecorationMap>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecorationMap>();
    }

    #[test]
    fn get_on_empty_map_returns_none() {
        let map = DecorationMap::new();
        assert!(map.get::<TypeInfo>(0).is_none());
    }

    #[test]
    fn get_retrieves_inserted_decoration() {
        let mut map = DecorationMap::new();
        map.insert(42, TypeInfo("bool".into()));

        let result = map.get::<TypeInfo>(42).expect("should find decoration");
        assert_eq!(result.0, "bool");
    }

    #[test]
    fn get_with_wrong_type_returns_none() {
        let mut map = DecorationMap::new();
        map.insert(42, TypeInfo("bool".into()));

        assert!(map.get::<Scope>(42).is_none());
    }

    #[test]
    fn get_with_wrong_node_returns_none() {
        let mut map = DecorationMap::new();
        map.insert(42, TypeInfo("bool".into()));

        assert!(map.get::<TypeInfo>(99).is_none());
    }

    #[test]
    fn get_all_returns_multiple_same_type() {
        let mut map = DecorationMap::new();
        map.insert(1, TypeInfo("i32".into()));
        map.insert(1, TypeInfo("u64".into()));

        let results = map.get_all::<TypeInfo>(1);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "i32");
        assert_eq!(results[1].0, "u64");
    }

    #[test]
    fn get_all_on_missing_node_returns_empty() {
        let map = DecorationMap::new();
        let results = map.get_all::<TypeInfo>(99);
        assert!(results.is_empty());
    }

    #[test]
    fn multiple_types_on_same_node() {
        let mut map = DecorationMap::new();
        map.insert(1, TypeInfo("i32".into()));
        map.insert(1, Scope(3));

        assert!(map.get::<TypeInfo>(1).is_some());
        assert!(map.get::<Scope>(1).is_some());
        assert_eq!(map.get::<Scope>(1).unwrap().0, 3);
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn insert_then_get_roundtrips(node_id: usize, value: u64) {
                let mut map = DecorationMap::new();
                map.insert(node_id, value);

                let result = map.get::<u64>(node_id);
                prop_assert_eq!(result, Some(&value));
            }

            #[test]
            fn get_on_different_node_returns_none(
                insert_id: usize,
                query_id: usize,
                value: u64,
            ) {
                prop_assume!(insert_id != query_id);
                let mut map = DecorationMap::new();
                map.insert(insert_id, value);

                prop_assert!(map.get::<u64>(query_id).is_none());
            }

            #[test]
            fn get_with_wrong_type_returns_none(node_id: usize, value: u64) {
                let mut map = DecorationMap::new();
                map.insert(node_id, value);

                prop_assert!(map.get::<i32>(node_id).is_none());
            }

            #[test]
            fn get_all_count_matches_insert_count(
                node_id: usize,
                values in proptest::collection::vec(any::<u32>(), 0..=20),
            ) {
                let mut map = DecorationMap::new();
                for v in &values {
                    map.insert(node_id, *v);
                }

                let results = map.get_all::<u32>(node_id);
                prop_assert_eq!(results.len(), values.len());
            }

            #[test]
            fn get_all_preserves_insertion_order(
                node_id: usize,
                values in proptest::collection::vec(any::<u32>(), 1..=10),
            ) {
                let mut map = DecorationMap::new();
                for v in &values {
                    map.insert(node_id, *v);
                }

                let results = map.get_all::<u32>(node_id);
                let result_values: Vec<u32> = results.into_iter().copied().collect();
                prop_assert_eq!(result_values, values);
            }

            #[test]
            fn different_types_do_not_interfere(node_id: usize, a: u32, b: i64) {
                let mut map = DecorationMap::new();
                map.insert(node_id, a);
                map.insert(node_id, b);

                prop_assert_eq!(map.get::<u32>(node_id), Some(&a));
                prop_assert_eq!(map.get::<i64>(node_id), Some(&b));
            }
        }
    }
}
