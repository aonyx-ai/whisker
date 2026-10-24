//! TypeScript support for whisker
//!
//! A TypeScript rule implements the generated [`TypeScriptLintPass`],
//! which has one method per node kind, and [`TypeScriptLintPassAdapter`]
//! bridges it to the boundary.
//!
//! TypeScript needs two grammars. `.ts` files parse with [`language`], and
//! `.tsx` files with [`tsx_language`], whose JSX syntax conflicts with the
//! angle-bracket type assertion. The trait comes from the union of both, so
//! a rule written for one kind of file runs on the other. A method for a
//! kind that only one grammar produces is never called on the other's trees.
//!
//! The default `provider` feature adds [`TypeScriptDecorationProvider`],
//! which type-checks the project with tsrs and records what the checker
//! knows in [`decorations`]. A plugin needs only the trait and the
//! decoration types, so it turns the feature off.

mod adapter;
pub mod decorations;
mod language_fingerprint;
#[cfg(feature = "provider")]
mod provider;

pub use adapter::TypeScriptLintPassAdapter;
pub use language_fingerprint::LANGUAGE_FINGERPRINT;
#[cfg(feature = "provider")]
pub use provider::TypeScriptDecorationProvider;

include!(concat!(env!("OUT_DIR"), "/typescript_lint_pass.rs"));

/// Returns the tree-sitter grammar for TypeScript files without JSX
///
/// # Examples
///
/// ```
/// let mut parser = tree_sitter::Parser::new();
/// parser.set_language(&whisker_typescript::language()).unwrap();
///
/// let tree = parser.parse("const a = <number>b;", None).unwrap();
///
/// assert!(!tree.root_node().has_error());
/// ```
pub fn language() -> tree_sitter::Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

/// Returns the tree-sitter grammar for TSX files
///
/// # Examples
///
/// ```
/// let mut parser = tree_sitter::Parser::new();
/// parser.set_language(&whisker_typescript::tsx_language()).unwrap();
///
/// let tree = parser.parse("const a = <div>hi</div>;", None).unwrap();
///
/// assert!(!tree.root_node().has_error());
/// ```
pub fn tsx_language() -> tree_sitter::Language {
    tree_sitter_typescript::LANGUAGE_TSX.into()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{DecoratedNode, DecoratedTree, Diagnostic, Language};

    use super::*;

    fn parse_with(grammar: &tree_sitter::Language, source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(grammar).unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(
            tree,
            Language::TypeScript,
            source.to_string(),
            PathBuf::from("test.ts"),
        )
    }

    fn walk_and_dispatch(node: &DecoratedNode<'_>, pass: &mut impl TypeScriptLintPass) {
        dispatch(pass, node);
        for child in node.named_children() {
            walk_and_dispatch(&child, pass);
        }
    }

    #[derive(Default)]
    struct KindCounter {
        comments: usize,
        expressions: usize,
        jsx_elements: usize,
        type_assertions: usize,
    }

    impl TypeScriptLintPass for KindCounter {
        fn check_comment(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            self.comments += 1;
            Vec::new()
        }

        fn check_expression(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            self.expressions += 1;
            Vec::new()
        }

        fn check_jsx_element(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            self.jsx_elements += 1;
            Vec::new()
        }

        fn check_type_assertion(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            self.type_assertions += 1;
            Vec::new()
        }
    }

    #[test]
    fn dispatch_calls_a_method_only_the_tsx_grammar_produces() {
        let tree = parse_with(&tsx_language(), "const a = <div>hi</div>;");
        let mut pass = KindCounter::default();

        walk_and_dispatch(&tree.root_node(), &mut pass);

        assert_eq!(pass.jsx_elements, 1);
        assert_eq!(pass.type_assertions, 0);
    }

    #[test]
    fn dispatch_calls_a_method_only_the_typescript_grammar_produces() {
        let tree = parse_with(&language(), "const a = <number>b;");
        let mut pass = KindCounter::default();

        walk_and_dispatch(&tree.root_node(), &mut pass);

        assert_eq!(pass.type_assertions, 1);
        assert_eq!(pass.jsx_elements, 0);
    }

    #[test]
    fn dispatch_calls_comment_method() {
        let tree = parse_with(&language(), "// one\n/* two */\n/** three */\nlet a = 1;");
        let mut pass = KindCounter::default();

        walk_and_dispatch(&tree.root_node(), &mut pass);

        assert_eq!(pass.comments, 3);
    }

    #[test]
    fn dispatch_calls_expression_supertype() {
        let tree = parse_with(&language(), "let a = 1 + 2;");
        let mut pass = KindCounter::default();

        walk_and_dispatch(&tree.root_node(), &mut pass);

        assert!(pass.expressions > 0);
    }

    #[test]
    fn language_parses_typescript_source() {
        let tree = parse_with(&language(), "function main(): void {}");

        let root = tree.root_node();

        assert_eq!(root.kind(), "program");
        assert!(!root.raw().has_error());
    }

    #[test]
    fn tsx_language_parses_jsx() {
        let tree = parse_with(&tsx_language(), "const a = <div className=\"x\" />;");

        let root = tree.root_node();

        assert!(!root.raw().has_error());
    }

    #[test]
    fn vendored_node_types_match_the_linked_grammars() {
        let typescript = include_str!("typescript-node-types.json");
        let tsx = include_str!("tsx-node-types.json");

        assert_eq!(typescript, tree_sitter_typescript::TYPESCRIPT_NODE_TYPES);
        assert_eq!(tsx, tree_sitter_typescript::TSX_NODE_TYPES);
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn dispatch_default_impl_never_produces_diagnostics(
                source in "(function [a-z]+\\(\\): void \\{\\}\n){1,5}",
            ) {
                struct Empty;
                impl TypeScriptLintPass for Empty {}

                fn walk_collecting(
                    node: &DecoratedNode<'_>,
                    pass: &mut impl TypeScriptLintPass,
                ) -> Vec<Diagnostic> {
                    let mut diagnostics = dispatch(pass, node);
                    for child in node.named_children() {
                        diagnostics.extend(walk_collecting(&child, pass));
                    }
                    diagnostics
                }

                let tree = parse_with(&language(), &source);

                let diagnostics = walk_collecting(&tree.root_node(), &mut Empty);

                prop_assert!(diagnostics.is_empty());
            }
        }
    }
}
