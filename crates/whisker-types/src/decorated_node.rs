use std::mem::offset_of;
use std::path::Path;
use std::sync::Arc;

use crate::{Decoration, DecorationMap, Span};

/// A tree-sitter node enriched with semantic decorations
///
/// This is the primary type that lint rules interact with. It provides
/// access to the tree-sitter node's structural information (kind, text,
/// children) and to semantic decorations attached by providers.
#[derive(Clone)]
pub struct DecoratedNode<'a> {
    node: tree_sitter::Node<'a>,
    source: &'a str,
    file: &'a Arc<Path>,
    decorations: &'a DecorationMap,
}

impl<'a> DecoratedNode<'a> {
    /// Creates a decorated node wrapping a tree-sitter node
    pub fn new(
        node: tree_sitter::Node<'a>,
        source: &'a str,
        file: &'a Arc<Path>,
        decorations: &'a DecorationMap,
    ) -> Self {
        Self {
            node,
            source,
            file,
            decorations,
        }
    }

    /// Returns the tree-sitter node kind (e.g. `"function_item"`)
    pub fn kind(&self) -> &str {
        self.node.kind()
    }

    /// Returns the source text covered by this node
    pub fn text(&self) -> &'a str {
        &self.source[self.node.byte_range()]
    }

    /// Returns the tree-sitter node ID, used as the decoration map key
    pub fn id(&self) -> usize {
        self.node.id()
    }

    /// Returns a [`Span`] covering this node's byte range
    ///
    /// The file path is reference-counted, so this is a cheap operation.
    pub fn span(&self) -> Span {
        Span::new(
            Arc::clone(self.file),
            self.node.start_byte(),
            self.node.end_byte(),
        )
    }

    /// Returns the number of named children
    pub fn named_child_count(&self) -> usize {
        self.node.named_child_count()
    }

    /// Returns the named child at the given index, if it exists
    pub fn named_child(&self, index: u32) -> Option<DecoratedNode<'a>> {
        self.node
            .named_child(index)
            .map(|child| DecoratedNode::new(child, self.source, self.file, self.decorations))
    }

    /// Returns a child node by its field name, if it exists
    pub fn child_by_field_name(&self, name: &str) -> Option<DecoratedNode<'a>> {
        self.node
            .child_by_field_name(name)
            .map(|child| DecoratedNode::new(child, self.source, self.file, self.decorations))
    }

    /// Returns the total number of children (named and anonymous)
    pub fn child_count(&self) -> usize {
        self.node.child_count()
    }

    /// Returns the child at the given index (named or anonymous)
    pub fn child(&self, index: u32) -> Option<DecoratedNode<'a>> {
        self.node
            .child(index)
            .map(|child| DecoratedNode::new(child, self.source, self.file, self.decorations))
    }

    /// Returns whether this is a named node (as opposed to an anonymous one)
    pub fn is_named(&self) -> bool {
        self.node.is_named()
    }

    /// Returns the parent node, if this is not the root
    pub fn parent(&self) -> Option<DecoratedNode<'a>> {
        self.node
            .parent()
            .map(|parent| DecoratedNode::new(parent, self.source, self.file, self.decorations))
    }

    /// Retrieves the first decoration of type `T` from this node
    pub fn decoration<T: Decoration>(&self) -> Option<&'a T> {
        self.decorations.get::<T>(self.node.id())
    }

    /// Retrieves all decorations of type `T` from this node
    pub fn decorations_of_type<T: Decoration>(&self) -> Vec<&'a T> {
        self.decorations.get_all::<T>(self.node.id())
    }

    /// Reads the decoration `D` from this node
    ///
    /// The shape of the result comes from `D` itself, not from this call: a
    /// decoration declared with cardinality `one` yields [`Option`] and one
    /// declared `many` yields [`Vec`]. Prefer this over [`decoration`] and
    /// [`decorations_of_type`], which accept any type and so cannot catch a
    /// rule reading a repeated decoration as though it were singular.
    ///
    /// [`decoration`]: DecoratedNode::decoration
    /// [`decorations_of_type`]: DecoratedNode::decorations_of_type
    pub fn get<D: Decoration>(&self) -> D::Ref<'a> {
        D::lookup(self)
    }

    /// Returns all named children of this node as a collected vec
    pub fn named_children(&self) -> Vec<DecoratedNode<'a>> {
        let count: u32 = self.node.named_child_count() as u32;
        (0..count)
            .filter_map(|i| {
                self.node.named_child(i).map(|child| {
                    DecoratedNode::new(child, self.source, self.file, self.decorations)
                })
            })
            .collect()
    }

    /// Returns the underlying tree-sitter node
    pub fn raw(&self) -> tree_sitter::Node<'a> {
        self.node
    }
}

impl std::fmt::Debug for DecoratedNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecoratedNode")
            .field("kind", &self.kind())
            .field("start_byte", &self.node.start_byte())
            .field("end_byte", &self.node.end_byte())
            .finish()
    }
}

/// The offsets of every field, in declaration order
///
/// The plugin handshake hashes these so a plugin that places a field
/// somewhere else is refused rather than trusted. They live beside the
/// struct, because a field added there has to be added here too.
pub(crate) const FIELD_OFFSETS: &[usize] = &[
    offset_of!(DecoratedNode<'static>, node),
    offset_of!(DecoratedNode<'static>, source),
    offset_of!(DecoratedNode<'static>, file),
    offset_of!(DecoratedNode<'static>, decorations),
];

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::DecorationKey;

    #[derive(Eq, PartialEq, Debug)]
    struct TestDeco(u32);

    unsafe impl Decoration for TestDeco {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::TestDeco"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[derive(Eq, PartialEq, Debug)]
    struct Missing;

    unsafe impl Decoration for Missing {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Missing"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[derive(Eq, PartialEq, Debug)]
    struct Value(u64);

    unsafe impl Decoration for Value {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Value"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    fn parse_tree(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecoratedNode<'_>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecoratedNode<'_>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecoratedNode<'_>>();
    }

    #[test]
    fn kind_returns_node_kind() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorations = DecorationMap::new();
        let file: Arc<Path> = PathBuf::from("test.rs").into();
        let root = DecoratedNode::new(tree.root_node(), source, &file, &decorations);

        assert_eq!(root.kind(), "source_file");
    }

    #[test]
    fn text_returns_source_slice() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorations = DecorationMap::new();
        let file: Arc<Path> = PathBuf::from("test.rs").into();
        let root = DecoratedNode::new(tree.root_node(), source, &file, &decorations);

        assert_eq!(root.text(), source);
    }

    #[test]
    fn span_covers_node_range() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorations = DecorationMap::new();
        let file: Arc<Path> = PathBuf::from("test.rs").into();
        let root = DecoratedNode::new(tree.root_node(), source, &file, &decorations);
        let span = root.span();

        assert_eq!(span.start(), 0);
        assert_eq!(span.end(), source.len());
    }

    #[test]
    fn named_child_returns_first_item() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorations = DecorationMap::new();
        let file: Arc<Path> = PathBuf::from("test.rs").into();
        let root = DecoratedNode::new(tree.root_node(), source, &file, &decorations);

        let first_child = root.named_child(0).expect("should have a child");
        assert_eq!(first_child.kind(), "function_item");
    }

    #[test]
    fn decoration_returns_attached_value() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let mut decorations = DecorationMap::new();
        let node_id = tree.root_node().id();
        decorations.insert(node_id, TestDeco(42));

        let file: Arc<Path> = PathBuf::from("test.rs").into();
        let root = DecoratedNode::new(tree.root_node(), source, &file, &decorations);
        let deco = root
            .decoration::<TestDeco>()
            .expect("should find decoration");
        assert_eq!(deco.0, 42);
    }

    #[test]
    fn decoration_returns_none_when_missing() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorations = DecorationMap::new();
        let file: Arc<Path> = PathBuf::from("test.rs").into();
        let root = DecoratedNode::new(tree.root_node(), source, &file, &decorations);

        assert!(root.decoration::<Missing>().is_none());
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn root_text_equals_source(source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}") {
                let tree = parse_tree(&source);
                let decorations = DecorationMap::new();
                let file: Arc<Path> = PathBuf::from("test.rs").into();
                let root = DecoratedNode::new(
                    tree.root_node(),
                    &source,
                    &file,
                    &decorations,
                );

                prop_assert_eq!(root.text(), source.as_str());
            }

            #[test]
            fn root_span_covers_full_source(source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}") {
                let tree = parse_tree(&source);
                let decorations = DecorationMap::new();
                let file: Arc<Path> = PathBuf::from("test.rs").into();
                let root = DecoratedNode::new(
                    tree.root_node(),
                    &source,
                    &file,
                    &decorations,
                );

                prop_assert_eq!(root.span().start(), 0);
                prop_assert_eq!(root.span().end(), source.len());
            }

            #[test]
            fn named_child_count_matches_named_children_len(source in "\\PC{0,200}") {
                let tree = parse_tree(&source);
                let decorations = DecorationMap::new();
                let file: Arc<Path> = PathBuf::from("test.rs").into();
                let root = DecoratedNode::new(
                    tree.root_node(),
                    &source,
                    &file,
                    &decorations,
                );

                prop_assert_eq!(
                    root.named_child_count(),
                    root.named_children().len()
                );
            }

            #[test]
            fn named_child_out_of_bounds_returns_none(
                index in 1000..=u32::MAX,
            ) {
                let source = "fn main() {}";
                let tree = parse_tree(source);
                let decorations = DecorationMap::new();
                let file: Arc<Path> = PathBuf::from("test.rs").into();
                let root = DecoratedNode::new(
                    tree.root_node(),
                    source,
                    &file,
                    &decorations,
                );

                prop_assert!(root.named_child(index).is_none());
            }

            #[test]
            fn decoration_roundtrips_through_node(value: u64) {
                let source = "fn main() {}";
                let tree = parse_tree(source);
                let mut decorations = DecorationMap::new();
                decorations.insert(tree.root_node().id(), Value(value));

                let file: Arc<Path> = PathBuf::from("test.rs").into();
                let root = DecoratedNode::new(
                    tree.root_node(),
                    source,
                    &file,
                    &decorations,
                );

                prop_assert_eq!(root.decoration::<Value>(), Some(&Value(value)));
            }
        }
    }
}
