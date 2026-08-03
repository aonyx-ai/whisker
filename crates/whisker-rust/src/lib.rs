mod adapter;
pub mod decorations;
mod provider;

pub use adapter::RustLintPassAdapter;
pub use provider::RustDecorationProvider;

include!(concat!(env!("OUT_DIR"), "/rust_lint_pass.rs"));

/// Returns the tree-sitter language for Rust
pub fn language() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{
        Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationMap, DecorationProvider,
        Diagnostic, RuleId, Severity,
    };

    use super::*;

    fn parse_rust(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(tree, source.to_string(), PathBuf::from("test.rs"))
    }

    #[test]
    fn trait_send_provider() {
        fn assert_send<T: Send>() {}
        assert_send::<RustDecorationProvider>();
    }

    #[test]
    fn trait_sync_provider() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RustDecorationProvider>();
    }

    #[test]
    fn trait_unpin_provider() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RustDecorationProvider>();
    }

    #[test]
    fn language_returns_valid_parser_language() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language())
            .expect("language should be valid for parser");
    }

    #[test]
    fn language_parses_rust_source() {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language()).unwrap();
        let tree = parser.parse("fn main() {}", None).unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn decorate_with_empty_provider_leaves_existing_decorations_intact() {
        let provider = RustDecorationProvider::empty();
        let mut tree = parse_rust("fn main() {}");
        let root_id = tree.root_node().id();
        let mut staged = DecorationMap::new();
        staged.insert(root_id, 7u32);
        tree.merge_decorations(staged);

        let coverage = provider.decorate(&tree).expect("should succeed");

        match coverage {
            Coverage::Covered(_) => panic!("an empty VFS cannot cover any file"),
            Coverage::NotCovered(CoverageGap::OutsideWorkspace { .. }) => {}
            Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
        }
        assert_eq!(tree.root_node().decoration::<u32>(), Some(&7));
    }

    #[test]
    fn decorate_with_empty_provider_reports_outside_workspace() {
        let provider = RustDecorationProvider::empty();
        let tree = parse_rust("");

        let coverage = provider.decorate(&tree).expect("should succeed");

        match coverage {
            Coverage::Covered(_) => panic!("an empty VFS cannot cover any file"),
            Coverage::NotCovered(CoverageGap::OutsideWorkspace { .. }) => {}
            Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
        }
    }

    #[test]
    fn name_returns_rust() {
        let provider = RustDecorationProvider::empty();

        let name = provider.name();

        assert_eq!(name.as_str(), "rust");
    }

    #[test]
    fn dispatch_calls_function_item_method() {
        struct FnCounter {
            count: usize,
        }
        impl RustLintPass for FnCounter {
            fn check_function_item(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                self.count += 1;
                Vec::new()
            }
        }

        let tree = parse_rust("fn main() {} fn helper() {}");
        let root = tree.root_node();
        let mut pass = FnCounter { count: 0 };

        for child in root.named_children() {
            dispatch(&mut pass, &child);
        }

        assert_eq!(pass.count, 2);
    }

    #[test]
    fn dispatch_calls_expression_supertype() {
        struct ExprCounter {
            count: usize,
        }
        impl RustLintPass for ExprCounter {
            fn check_expression(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                self.count += 1;
                Vec::new()
            }
        }

        let tree = parse_rust("fn main() { let x = 1 + 2; }");
        let root = tree.root_node();
        let mut pass = ExprCounter { count: 0 };

        fn walk_and_dispatch(node: &DecoratedNode<'_>, pass: &mut impl RustLintPass) {
            dispatch(pass, node);
            for child in node.named_children() {
                walk_and_dispatch(&child, pass);
            }
        }

        walk_and_dispatch(&root, &mut pass);

        assert!(
            pass.count > 0,
            "should have dispatched expression supertype at least once"
        );
    }

    #[test]
    fn dispatch_calls_both_concrete_and_supertype() {
        struct DualCounter {
            concrete: usize,
            supertype: usize,
        }
        impl RustLintPass for DualCounter {
            fn check_integer_literal(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                self.concrete += 1;
                Vec::new()
            }
            fn check_literal(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                self.supertype += 1;
                Vec::new()
            }
        }

        let tree = parse_rust("fn main() { let _ = 42; }");
        let root = tree.root_node();
        let mut pass = DualCounter {
            concrete: 0,
            supertype: 0,
        };

        fn walk_and_dispatch(node: &DecoratedNode<'_>, pass: &mut impl RustLintPass) {
            dispatch(pass, node);
            for child in node.named_children() {
                walk_and_dispatch(&child, pass);
            }
        }

        walk_and_dispatch(&root, &mut pass);

        assert!(
            pass.concrete > 0,
            "integer_literal should have been dispatched"
        );
        assert!(
            pass.supertype > 0,
            "literal supertype should have been dispatched"
        );
    }

    #[test]
    fn dispatch_returns_diagnostics_from_pass() {
        struct AlwaysWarn;
        impl RustLintPass for AlwaysWarn {
            fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                vec![Diagnostic::new(
                    RuleId("test.warn"),
                    Severity::Warn,
                    "found function".into(),
                    node.span(),
                )]
            }
        }

        let tree = parse_rust("fn main() {}");
        let root = tree.root_node();
        let fn_item = root.named_child(0).expect("should have function_item");
        let mut pass = AlwaysWarn;

        let diagnostics = dispatch(&mut pass, &fn_item);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id(), RuleId("test.warn"));
    }

    #[test]
    fn dispatch_on_unknown_node_returns_empty() {
        struct NoOp;
        impl RustLintPass for NoOp {}

        let tree = parse_rust("fn main() {}");
        let root = tree.root_node();
        let mut pass = NoOp;

        let diagnostics = dispatch(&mut pass, &root);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn default_impl_returns_empty_for_all_methods() {
        struct Empty;
        impl RustLintPass for Empty {}

        let tree = parse_rust("fn main() { let x = 1 + 2; }");
        let root = tree.root_node();
        let mut pass = Empty;

        fn walk_collecting(
            node: &DecoratedNode<'_>,
            pass: &mut impl RustLintPass,
        ) -> Vec<Diagnostic> {
            let mut diags = dispatch(pass, node);
            for child in node.named_children() {
                diags.extend(walk_collecting(&child, pass));
            }
            diags
        }

        let diagnostics = walk_collecting(&root, &mut pass);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn dispatch_handles_match_arm_node() {
        struct MatchArmChecker {
            found: bool,
        }
        impl RustLintPass for MatchArmChecker {
            fn check_match_arm(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                self.found = true;
                Vec::new()
            }
        }

        let tree = parse_rust("fn f() { match 1 { 0 => {} _ => {} } }");
        let root = tree.root_node();
        let mut pass = MatchArmChecker { found: false };

        fn walk_and_dispatch(node: &DecoratedNode<'_>, pass: &mut impl RustLintPass) {
            dispatch(pass, node);
            for child in node.named_children() {
                walk_and_dispatch(&child, pass);
            }
        }

        walk_and_dispatch(&root, &mut pass);
        assert!(pass.found, "should have found match_arm nodes");
    }

    #[test]
    fn dispatch_handles_macro_invocation() {
        struct MacroChecker {
            found: bool,
        }
        impl RustLintPass for MacroChecker {
            fn check_macro_invocation(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                self.found = true;
                Vec::new()
            }
        }

        let tree = parse_rust("fn f() { println!(\"hello\"); }");
        let root = tree.root_node();
        let mut pass = MacroChecker { found: false };

        fn walk_and_dispatch(node: &DecoratedNode<'_>, pass: &mut impl RustLintPass) {
            dispatch(pass, node);
            for child in node.named_children() {
                walk_and_dispatch(&child, pass);
            }
        }

        walk_and_dispatch(&root, &mut pass);
        assert!(pass.found, "should have found macro_invocation node");
    }

    #[test]
    fn dispatch_handles_attribute_item() {
        struct AttrChecker {
            found: bool,
        }
        impl RustLintPass for AttrChecker {
            fn check_attribute_item(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                self.found = true;
                Vec::new()
            }
        }

        let tree = parse_rust("#[derive(Debug)]\nstruct Foo;");
        let root = tree.root_node();
        let mut pass = AttrChecker { found: false };

        fn walk_and_dispatch(node: &DecoratedNode<'_>, pass: &mut impl RustLintPass) {
            dispatch(pass, node);
            for child in node.named_children() {
                walk_and_dispatch(&child, pass);
            }
        }

        walk_and_dispatch(&root, &mut pass);
        assert!(pass.found, "should have found attribute_item node");
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn dispatch_default_impl_never_produces_diagnostics(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){1,5}",
            ) {
                struct Empty;
                impl RustLintPass for Empty {}

                let tree = parse_rust(&source);
                let root = tree.root_node();
                let mut pass = Empty;

                fn walk_collecting(
                    node: &DecoratedNode<'_>,
                    pass: &mut impl RustLintPass,
                ) -> Vec<Diagnostic> {
                    let mut diags = dispatch(pass, node);
                    for child in node.named_children() {
                        diags.extend(walk_collecting(&child, pass));
                    }
                    diags
                }

                let diagnostics = walk_collecting(&root, &mut pass);
                prop_assert!(diagnostics.is_empty());
            }

            #[test]
            fn dispatch_function_counter_matches_fn_count(
                count in 1..=10usize,
            ) {
                struct FnCounter(usize);
                impl RustLintPass for FnCounter {
                    fn check_function_item(
                        &mut self,
                        _node: &DecoratedNode<'_>,
                    ) -> Vec<Diagnostic> {
                        self.0 += 1;
                        Vec::new()
                    }
                }

                let source: String = (0..count)
                    .map(|i| format!("fn f{i}() {{}}\n"))
                    .collect();

                let tree = parse_rust(&source);
                let root = tree.root_node();
                let mut pass = FnCounter(0);

                fn walk_and_dispatch(
                    node: &DecoratedNode<'_>,
                    pass: &mut impl RustLintPass,
                ) {
                    dispatch(pass, node);
                    for child in node.named_children() {
                        walk_and_dispatch(&child, pass);
                    }
                }

                walk_and_dispatch(&root, &mut pass);
                prop_assert_eq!(pass.0, count);
            }
        }
    }
}
