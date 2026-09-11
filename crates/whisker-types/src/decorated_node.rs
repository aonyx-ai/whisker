use stabby::abi::StableLike;
use stabby::str::Str;

use crate::ts_node::TsNode;
use crate::{Decoration, DecorationLookup, FilePath, Span};

/// A tree-sitter node enriched with semantic decorations
///
/// This is the primary type that lint rules interact with. It provides
/// access to the tree-sitter node's structural information (kind, text,
/// children) and to semantic decorations attached by providers.
/// Copying one moves borrowed data and nothing else. Every field is a
/// reference, a function pointer, or a `tree_sitter::Node`, which is
/// itself a `#[repr(transparent)]` wrapper around a C struct of pointers.
/// The [`FilePath`] sits behind a reference, so a copy reaches no
/// allocation and moves no refcount.
///
/// This is `Copy` so that a rule walking siblings and children writes what
/// it means. Without it, reading a node out of the `Vec` that
/// [`DecoratedNode::named_children`] returns forces a clone that copies
/// exactly what a move would and reads as though it costs something.
///
/// A node crosses the plugin boundary with every check a pass runs, so
/// stabby lays it out. The tree-sitter node inside is a C struct that
/// stabby cannot see into, so [`StableLike`] carries it with a copy of
/// that struct's layout. A plugin reads decorations through a
/// [`DecorationLookup`], two borrowed slices it searches itself.
///
/// [`StableLike`]: stabby::abi::StableLike
#[stabby::stabby]
#[derive(Copy, Clone)]
pub struct DecoratedNode<'a> {
    node: StableLike<tree_sitter::Node<'a>, TsNode>,
    source: Str<'a>,
    file: &'a FilePath,
    decorations: DecorationLookup<'a>,
}

impl<'a> DecoratedNode<'a> {
    /// Creates a decorated node wrapping a tree-sitter node
    pub fn new(
        node: tree_sitter::Node<'a>,
        source: &'a str,
        file: &'a FilePath,
        decorations: DecorationLookup<'a>,
    ) -> Self {
        Self {
            node: StableLike::new(node),
            source: Str::new(source),
            file,
            decorations,
        }
    }

    /// Returns the tree-sitter node this wraps
    ///
    /// Stabby cannot tell that the inner type is FFI-safe, so it marks the
    /// read unsafe. `tree_sitter::Node` is `#[repr(transparent)]` over a
    /// `#[repr(C)]` struct, so the bytes the host wrote read back as the
    /// same node under any compiler.
    fn inner(&self) -> tree_sitter::Node<'a> {
        // SAFETY: `TsNode` is a copy of `TSNode`'s layout, checked for
        // size and alignment where `StableLike::new` builds the wrapper,
        // and `tree_sitter::Node` is `#[repr(transparent)]` over `TSNode`.
        unsafe { *self.node.as_ref_unchecked() }
    }

    /// Wraps another node of the same tree
    fn sibling(&self, node: tree_sitter::Node<'a>) -> Self {
        Self {
            node: StableLike::new(node),
            ..*self
        }
    }

    /// Returns the tree-sitter node kind (e.g. `"function_item"`)
    pub fn kind(&self) -> &'a str {
        self.inner().kind()
    }

    /// Returns the source text covered by this node
    pub fn text(&self) -> &'a str {
        &self.source.as_str()[self.inner().byte_range()]
    }

    /// Returns the tree-sitter node ID, used as the decoration map key
    pub fn id(&self) -> usize {
        self.inner().id()
    }

    /// Returns a [`Span`] covering this node's byte range
    ///
    /// The file path is reference-counted, so this is a cheap operation.
    pub fn span(&self) -> Span {
        let node = self.inner();

        Span::new(self.file.clone(), node.start_byte(), node.end_byte())
    }

    /// Returns the number of named children
    pub fn named_child_count(&self) -> usize {
        self.inner().named_child_count()
    }

    /// Returns the named child at the given index, if it exists
    pub fn named_child(&self, index: u32) -> Option<DecoratedNode<'a>> {
        self.inner()
            .named_child(index)
            .map(|child| self.sibling(child))
    }

    /// Returns a child node by its field name, if it exists
    pub fn child_by_field_name(&self, name: &str) -> Option<DecoratedNode<'a>> {
        self.inner()
            .child_by_field_name(name)
            .map(|child| self.sibling(child))
    }

    /// Returns the total number of children (named and anonymous)
    pub fn child_count(&self) -> usize {
        self.inner().child_count() as usize
    }

    /// Returns the child at the given index (named or anonymous)
    pub fn child(&self, index: u32) -> Option<DecoratedNode<'a>> {
        self.inner().child(index).map(|child| self.sibling(child))
    }

    /// Returns whether this is a named node (as opposed to an anonymous one)
    pub fn is_named(&self) -> bool {
        self.inner().is_named()
    }

    /// Returns the parent node, if this is not the root
    pub fn parent(&self) -> Option<DecoratedNode<'a>> {
        self.inner().parent().map(|parent| self.sibling(parent))
    }

    /// Retrieves the first decoration of type `T` from this node
    pub fn decoration<T: Decoration>(&self) -> Option<&'a T> {
        self.decorations.get::<T>(self.id(), 0)
    }

    /// Retrieves all decorations of type `T` from this node
    pub fn decorations_of_type<T: Decoration>(&self) -> Vec<&'a T> {
        self.decorations.get_all::<T>(self.id())
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
        let node = self.inner();
        let count: u32 = node.named_child_count() as u32;
        (0..count)
            .filter_map(|i| node.named_child(i).map(|child| self.sibling(child)))
            .collect()
    }

    /// Returns the underlying tree-sitter node
    pub fn raw(&self) -> tree_sitter::Node<'a> {
        self.inner()
    }
}

impl std::fmt::Debug for DecoratedNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let node = self.inner();

        f.debug_struct("DecoratedNode")
            .field("kind", &node.kind())
            .field("start_byte", &node.start_byte())
            .field("end_byte", &node.end_byte())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecorationKey, DecorationMap};

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct TestDeco(u32);

    impl Decoration for TestDeco {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::TestDeco"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Missing;

    impl Decoration for Missing {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Missing"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    #[stabby::stabby]
    #[derive(Eq, PartialEq, Debug)]
    struct Value(u64);

    impl Decoration for Value {
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
    fn decorations_of_type_returns_every_value_in_order() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let mut decorations = DecorationMap::new();
        let node_id = tree.root_node().id();
        decorations.insert(node_id, TestDeco(1));
        decorations.insert(node_id, Value(9));
        decorations.insert(node_id, TestDeco(2));
        let file = FilePath::from("test.rs");
        let root = DecoratedNode::new(tree.root_node(), source, &file, decorations.lookup());

        let found = root.decorations_of_type::<TestDeco>();

        assert_eq!(found, vec![&TestDeco(1), &TestDeco(2)]);
    }

    #[test]
    fn kind_returns_node_kind() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorations = DecorationMap::new();
        let file = FilePath::from("test.rs");
        let root = DecoratedNode::new(tree.root_node(), source, &file, decorations.lookup());

        assert_eq!(root.kind(), "source_file");
    }

    #[test]
    fn text_returns_source_slice() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorations = DecorationMap::new();
        let file = FilePath::from("test.rs");
        let root = DecoratedNode::new(tree.root_node(), source, &file, decorations.lookup());

        assert_eq!(root.text(), source);
    }

    #[test]
    fn span_covers_node_range() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorations = DecorationMap::new();
        let file = FilePath::from("test.rs");
        let root = DecoratedNode::new(tree.root_node(), source, &file, decorations.lookup());
        let span = root.span();

        assert_eq!(span.start(), 0);
        assert_eq!(span.end(), source.len());
    }

    #[test]
    fn named_child_returns_first_item() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorations = DecorationMap::new();
        let file = FilePath::from("test.rs");
        let root = DecoratedNode::new(tree.root_node(), source, &file, decorations.lookup());

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

        let file = FilePath::from("test.rs");
        let root = DecoratedNode::new(tree.root_node(), source, &file, decorations.lookup());
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
        let file = FilePath::from("test.rs");
        let root = DecoratedNode::new(tree.root_node(), source, &file, decorations.lookup());

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
                let file = FilePath::from("test.rs");
                let root = DecoratedNode::new(
                    tree.root_node(),
                    &source,
                    &file,
                    decorations.lookup(),
                );

                prop_assert_eq!(root.text(), source.as_str());
            }

            #[test]
            fn root_span_covers_full_source(source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}") {
                let tree = parse_tree(&source);
                let decorations = DecorationMap::new();
                let file = FilePath::from("test.rs");
                let root = DecoratedNode::new(
                    tree.root_node(),
                    &source,
                    &file,
                    decorations.lookup(),
                );

                prop_assert_eq!(root.span().start(), 0);
                prop_assert_eq!(root.span().end(), source.len());
            }

            #[test]
            fn named_child_count_matches_named_children_len(source in "\\PC{0,200}") {
                let tree = parse_tree(&source);
                let decorations = DecorationMap::new();
                let file = FilePath::from("test.rs");
                let root = DecoratedNode::new(
                    tree.root_node(),
                    &source,
                    &file,
                    decorations.lookup(),
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
                let file = FilePath::from("test.rs");
                let root = DecoratedNode::new(
                    tree.root_node(),
                    source,
                    &file,
                    decorations.lookup(),
                );

                prop_assert!(root.named_child(index).is_none());
            }

            #[test]
            fn decoration_roundtrips_through_node(value: u64) {
                let source = "fn main() {}";
                let tree = parse_tree(source);
                let mut decorations = DecorationMap::new();
                decorations.insert(tree.root_node().id(), Value(value));

                let file = FilePath::from("test.rs");
                let root = DecoratedNode::new(
                    tree.root_node(),
                    source,
                    &file,
                    decorations.lookup(),
                );

                prop_assert_eq!(root.decoration::<Value>(), Some(&Value(value)));
            }
        }
    }
}
