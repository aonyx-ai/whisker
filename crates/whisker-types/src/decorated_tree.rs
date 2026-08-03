use std::path::Path;
use std::sync::Arc;

use crate::{DecoratedNode, DecorationMap};

/// A parsed syntax tree with an overlay of semantic decorations
///
/// Wraps a tree-sitter [`Tree`] together with its source text, file path,
/// and a [`DecorationMap`] that providers populate with per-node semantic
/// information. The tree and decorations together form the input to lint
/// rules.
///
/// [`Tree`]: tree_sitter::Tree
pub struct DecoratedTree {
    tree: tree_sitter::Tree,
    source: String,
    file: Arc<Path>,
    decorations: DecorationMap,
}

impl DecoratedTree {
    /// Creates a decorated tree from a parsed tree-sitter tree
    pub fn new(tree: tree_sitter::Tree, source: String, file: impl Into<Arc<Path>>) -> Self {
        Self {
            tree,
            source,
            file: file.into(),
            decorations: DecorationMap::new(),
        }
    }

    /// Returns a mutable reference to the decoration map for test helpers
    /// that hand-build a decorated tree
    ///
    /// A [`DecorationProvider`] cannot reach this: it is handed the tree
    /// immutably and returns its decorations instead, which the pipeline
    /// applies with [`DecoratedTree::merge_decorations`]. This escape hatch
    /// exists so a test can stage a decoration without loading a toolchain.
    ///
    /// [`DecorationProvider`]: crate::DecorationProvider
    pub fn decorations_mut(&mut self) -> &mut DecorationMap {
        &mut self.decorations
    }

    /// Merges provider decorations into this tree
    ///
    /// This is the only path available to a [`DecorationProvider`], which
    /// receives the tree immutably, and the pipeline calls it only after
    /// establishing that some provider claimed the file.
    ///
    /// [`DecorationProvider`]: crate::DecorationProvider
    pub fn merge_decorations(&mut self, decorations: DecorationMap) {
        self.decorations.merge(decorations);
    }

    /// Returns the root node wrapped as a [`DecoratedNode`]
    pub fn root_node(&self) -> DecoratedNode<'_> {
        DecoratedNode::new(
            self.tree.root_node(),
            &self.source,
            &self.file,
            &self.decorations,
        )
    }

    /// Returns the source text of the file
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the file path
    pub fn file(&self) -> &Path {
        &self.file
    }
}

impl std::fmt::Debug for DecoratedTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecoratedTree")
            .field("file", &self.file)
            .field("source_len", &self.source.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

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
        assert_send::<DecoratedTree>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecoratedTree>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecoratedTree>();
    }

    #[test]
    fn merge_decorations_makes_decorations_visible_on_nodes() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let mut decorated = DecoratedTree::new(tree, source.into(), PathBuf::from("test.rs"));
        let root_id = decorated.root_node().id();
        let mut decorations = DecorationMap::new();
        decorations.insert(root_id, 7u32);

        decorated.merge_decorations(decorations);

        assert_eq!(decorated.root_node().decoration::<u32>(), Some(&7));
    }

    #[test]
    fn merge_decorations_twice_keeps_first_of_a_type() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let mut decorated = DecoratedTree::new(tree, source.into(), PathBuf::from("test.rs"));
        let root_id = decorated.root_node().id();
        let mut first = DecorationMap::new();
        first.insert(root_id, 1u32);
        let mut second = DecorationMap::new();
        second.insert(root_id, 2u32);

        decorated.merge_decorations(first);
        decorated.merge_decorations(second);

        assert_eq!(decorated.root_node().decoration::<u32>(), Some(&1));
        assert_eq!(
            decorated.root_node().decorations_of_type::<u32>(),
            vec![&1, &2]
        );
    }

    #[test]
    fn root_node_kind_is_source_file() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorated = DecoratedTree::new(tree, source.into(), PathBuf::from("test.rs"));

        assert_eq!(decorated.root_node().kind(), "source_file");
    }

    #[test]
    fn source_returns_original_text() {
        let source = "fn main() {}";
        let tree = parse_tree(source);
        let decorated = DecoratedTree::new(tree, source.into(), PathBuf::from("test.rs"));

        assert_eq!(decorated.source(), source);
    }

    #[test]
    fn file_returns_path() {
        let source = "";
        let tree = parse_tree(source);
        let decorated = DecoratedTree::new(tree, source.into(), PathBuf::from("test.rs"));

        assert_eq!(decorated.file(), Path::new("test.rs"));
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn source_roundtrips(source in "\\PC{0,200}") {
                let tree = parse_tree(&source);
                let decorated = DecoratedTree::new(
                    tree,
                    source.clone(),
                    PathBuf::from("test.rs"),
                );

                prop_assert_eq!(decorated.source(), source);
            }

            #[test]
            fn file_roundtrips(file in "[a-z]{1,10}\\.rs") {
                let tree = parse_tree("");
                let decorated = DecoratedTree::new(
                    tree,
                    String::new(),
                    PathBuf::from(&file),
                );

                prop_assert_eq!(decorated.file(), Path::new(&file));
            }

            #[test]
            fn root_is_source_file_or_error(source in "\\PC{0,200}") {
                let tree = parse_tree(&source);
                let decorated = DecoratedTree::new(
                    tree,
                    source,
                    PathBuf::from("test.rs"),
                );

                let root = decorated.root_node();
                let kind = root.kind();
                prop_assert!(
                    kind == "source_file" || kind == "ERROR",
                    "unexpected root kind: {kind}"
                );
            }
        }
    }
}
