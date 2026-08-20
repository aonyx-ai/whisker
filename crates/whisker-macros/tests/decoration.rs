//! Exercises the derived `Decoration` implementations against a real tree.
//!
//! The unit tests in the crate itself check the generated tokens; these check
//! that the generated code compiles and that a node reads decorations back in
//! the shape the type declares.

use std::path::PathBuf;

use whisker_macros::Decoration;
use whisker_types::{DecoratedTree, Decoration};

#[derive(Decoration)]
#[decoration(cardinality = "one")]
struct ResolvedType(&'static str);

#[derive(Decoration)]
#[decoration(cardinality = "many")]
struct TraitImpl(&'static str);

fn parse(source: &str) -> DecoratedTree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("language should be valid");
    let tree = parser.parse(source, None).expect("should parse");

    DecoratedTree::new(tree, source.to_string(), PathBuf::from("test.rs"))
}

#[test]
fn many_cardinality_reads_back_every_instance_in_order() {
    let mut tree = parse("fn main() {}");
    let id = tree.root_node().id();
    tree.decorations_mut().insert(id, TraitImpl("Debug"));
    tree.decorations_mut().insert(id, TraitImpl("Clone"));

    let found = tree.root_node().get::<TraitImpl>();

    assert_eq!(found.len(), 2);
    assert_eq!(found[0].0, "Debug");
    assert_eq!(found[1].0, "Clone");
}

#[test]
fn many_cardinality_when_absent_reads_back_empty() {
    let tree = parse("fn main() {}");

    let found = tree.root_node().get::<TraitImpl>();

    assert!(found.is_empty());
}

#[test]
fn one_cardinality_reads_back_a_single_value() {
    let mut tree = parse("fn main() {}");
    let id = tree.root_node().id();
    tree.decorations_mut().insert(id, ResolvedType("u32"));

    let found = tree.root_node().get::<ResolvedType>();

    assert_eq!(found.expect("should be present").0, "u32");
}

#[test]
fn one_cardinality_when_absent_reads_back_none() {
    let tree = parse("fn main() {}");

    let found = tree.root_node().get::<ResolvedType>();

    assert!(found.is_none());
}

#[test]
fn decorations_are_keyed_per_node() {
    let source = "fn main() {}";
    let mut tree = parse(source);
    let root_id = tree.root_node().id();
    tree.decorations_mut().insert(root_id, ResolvedType("root"));

    let child = tree
        .root_node()
        .named_child(0)
        .expect("should have a function item");

    assert_eq!(
        tree.root_node()
            .get::<ResolvedType>()
            .expect("root should be decorated")
            .0,
        "root"
    );
    assert!(child.get::<ResolvedType>().is_none());
}

#[test]
fn lookup_can_be_called_through_the_trait() {
    let mut tree = parse("fn main() {}");
    let id = tree.root_node().id();
    tree.decorations_mut().insert(id, ResolvedType("u32"));

    let found = ResolvedType::lookup(&tree.root_node());

    assert_eq!(found.expect("should be present").0, "u32");
}

/// Two types of one name in two function bodies must stay apart
///
/// `module_path!` cannot see a function body, so a key built from the
/// module path and the name alone would be the same for both, and the map
/// would hand a lookup of one the other's memory.
#[test]
fn same_named_types_in_two_functions_do_not_collide() {
    fn decorate(tree: &mut DecoratedTree) -> u64 {
        #[derive(Decoration)]
        #[decoration(cardinality = "one")]
        struct Local(u64);

        let id = tree.root_node().id();
        tree.decorations_mut().insert(id, Local(7));

        tree.root_node().get::<Local>().expect("should read back").0
    }

    #[derive(Decoration)]
    #[decoration(cardinality = "one")]
    struct Local;

    let mut tree = parse("fn main() {}");

    let decorated = decorate(&mut tree);

    assert_eq!(decorated, 7);
    assert!(tree.root_node().get::<Local>().is_none());
}
