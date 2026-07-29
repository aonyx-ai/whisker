use std::any::Any;

use crate::DecoratedNode;

/// A semantic annotation that a provider can attach to syntax nodes
///
/// Decorations carry information a syntax tree cannot supply on its own —
/// resolved types, signatures, trait memberships — which a
/// [`DecorationProvider`] obtains from a language toolchain and records
/// against individual nodes.
///
/// Implementations fix their own cardinality through [`Ref`]. A decoration a
/// provider records at most once per node reads back as [`Option`]; one it may
/// record repeatedly reads back as [`Vec`]. Because that choice travels with
/// the type rather than the call site, a rule cannot ask for a single value
/// where the provider records a list, and the mismatch is a compile error
/// rather than a silently dropped decoration.
///
/// Derive this rather than writing it by hand:
///
/// ```ignore
/// #[derive(Decoration)]
/// #[decoration(cardinality = "one")]
/// pub struct ResolvedType { /* … */ }
/// ```
///
/// # Examples
///
/// ```
/// use whisker_types::{DecoratedNode, Decoration};
///
/// struct Signature(String);
///
/// impl Decoration for Signature {
///     type Ref<'a> = Option<&'a Self>;
///
///     fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
///         node.decoration::<Self>()
///     }
/// }
/// ```
///
/// [`DecorationProvider`]: crate::DecorationProvider
/// [`Ref`]: Decoration::Ref
pub trait Decoration: Any + Send + Sync + Sized {
    /// What a lookup of this decoration yields
    ///
    /// Use `Option<&'a Self>` for a decoration recorded at most once per node
    /// and `Vec<&'a Self>` for one recorded any number of times.
    type Ref<'a>;

    /// Reads this decoration from `node`
    ///
    /// Prefer [`DecoratedNode::get`], which calls this.
    fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a>;
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::DecoratedTree;

    struct Single(u32);

    impl Decoration for Single {
        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    struct Repeated(u32);

    impl Decoration for Repeated {
        type Ref<'a> = Vec<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decorations_of_type::<Self>()
        }
    }

    fn parse(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("language should be valid");
        let tree = parser.parse(source, None).expect("should parse");

        DecoratedTree::new(tree, source.to_string(), PathBuf::from("test.rs"))
    }

    #[test]
    fn get_with_many_cardinality_returns_every_instance() {
        let mut tree = parse("fn main() {}");
        let id = tree.root_node().id();
        tree.decorations_mut().insert(id, Repeated(1));
        tree.decorations_mut().insert(id, Repeated(2));

        let found = tree.root_node().get::<Repeated>();

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, 1);
        assert_eq!(found[1].0, 2);
    }

    #[test]
    fn get_with_many_cardinality_when_absent_returns_empty() {
        let tree = parse("fn main() {}");

        let found = tree.root_node().get::<Repeated>();

        assert!(found.is_empty());
    }

    #[test]
    fn get_with_one_cardinality_returns_the_decoration() {
        let mut tree = parse("fn main() {}");
        let id = tree.root_node().id();
        tree.decorations_mut().insert(id, Single(7));

        let found = tree.root_node().get::<Single>();

        assert_eq!(found.expect("should be present").0, 7);
    }

    #[test]
    fn get_with_one_cardinality_when_absent_returns_none() {
        let tree = parse("fn main() {}");

        let found = tree.root_node().get::<Single>();

        assert!(found.is_none());
    }
}
