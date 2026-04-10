use whisker_types::{DecoratedNode, DecoratedTree, Diagnostic, LintPass};

/// Walks a decorated syntax tree and collects diagnostics from lint passes
///
/// Performs a depth-first traversal of all named nodes, calling each lint
/// pass for every node visited.
// r[impl core.tree-walker]
pub fn walk(tree: &DecoratedTree, passes: &mut [Box<dyn LintPass>]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    visit_node(&tree.root_node(), passes, &mut diagnostics);
    diagnostics
}

fn visit_node(
    node: &DecoratedNode<'_>,
    passes: &mut [Box<dyn LintPass>],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if node.is_named() {
        for pass in passes.iter_mut() {
            diagnostics.extend(pass.check_node(node));
        }
    }

    for child in node.named_children() {
        visit_node(&child, passes, diagnostics);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use whisker_types::DecoratedTree;

    use super::*;

    fn parse_rust(source: &str) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        DecoratedTree::new(tree, source.to_string(), PathBuf::from("test.rs"))
    }

    #[test]
    fn walk_with_empty_passes_returns_empty() {
        let tree = parse_rust("fn main() {}");
        let diagnostics = walk(&tree, &mut Vec::new());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn walk_visits_all_named_nodes() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Counter;
        impl LintPass for Counter {
            fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                COUNT.fetch_add(1, Ordering::Relaxed);
                Vec::new()
            }
        }

        COUNT.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn main() {}");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(Counter)];
        walk(&tree, &mut passes);

        assert!(COUNT.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn walk_on_empty_source_returns_empty() {
        let tree = parse_rust("");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(CounterPass(0))];
        let diagnostics = walk(&tree, &mut passes);
        assert!(diagnostics.is_empty());
    }

    struct CounterPass(usize);
    impl LintPass for CounterPass {
        fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            self.0 += 1;
            Vec::new()
        }
    }

    #[test]
    fn walk_visits_nested_nodes() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static NESTED_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct KindCounter;
        impl LintPass for KindCounter {
            fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                NESTED_COUNT.fetch_add(1, Ordering::Relaxed);
                Vec::new()
            }
        }

        NESTED_COUNT.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn main() { let x = 1; }");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(KindCounter)];
        walk(&tree, &mut passes);

        let count = NESTED_COUNT.load(Ordering::Relaxed);
        assert!(count > 1, "should visit multiple nested nodes, got {count}");
    }

    #[test]
    fn walk_visits_deeply_nested_code() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static DEEP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct DepthCounter;
        impl LintPass for DepthCounter {
            fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                DEEP_COUNT.fetch_add(1, Ordering::Relaxed);
                Vec::new()
            }
        }

        DEEP_COUNT.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn f() { if true { if true { if true { let x = 1; } } } }");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(DepthCounter)];
        walk(&tree, &mut passes);

        let count = DEEP_COUNT.load(Ordering::Relaxed);
        assert!(
            count >= 4,
            "deeply nested source should visit at least 4 nodes, got {count}"
        );
    }

    #[test]
    fn walk_multiple_passes_each_see_same_nodes() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static PASS_A: AtomicUsize = AtomicUsize::new(0);
        static PASS_B: AtomicUsize = AtomicUsize::new(0);

        struct CounterA;
        impl LintPass for CounterA {
            fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                PASS_A.fetch_add(1, Ordering::Relaxed);
                Vec::new()
            }
        }

        struct CounterB;
        impl LintPass for CounterB {
            fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                PASS_B.fetch_add(1, Ordering::Relaxed);
                Vec::new()
            }
        }

        PASS_A.store(0, Ordering::Relaxed);
        PASS_B.store(0, Ordering::Relaxed);

        let tree = parse_rust("fn a() {} fn b() {}");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(CounterA), Box::new(CounterB)];
        walk(&tree, &mut passes);

        let a = PASS_A.load(Ordering::Relaxed);
        let b = PASS_B.load(Ordering::Relaxed);
        assert!(a > 0, "pass A should have been called");
        assert_eq!(a, b, "both passes should see the same number of nodes");
    }

    #[test]
    fn walk_collects_diagnostics_from_multiple_passes() {
        use whisker_types::{RuleId, Severity};

        struct WarnOnFn(&'static str);
        impl LintPass for WarnOnFn {
            fn check_node(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                if node.kind() == "function_item" {
                    vec![Diagnostic::new(
                        RuleId(self.0),
                        Severity::Warn,
                        format!("{} found fn", self.0),
                        node.span(),
                    )]
                } else {
                    Vec::new()
                }
            }
        }

        let tree = parse_rust("fn main() {}");
        let mut passes: Vec<Box<dyn LintPass>> =
            vec![Box::new(WarnOnFn("pass.a")), Box::new(WarnOnFn("pass.b"))];
        let diagnostics = walk(&tree, &mut passes);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].rule_id(), RuleId("pass.a"));
        assert_eq!(diagnostics[1].rule_id(), RuleId("pass.b"));
    }

    #[test]
    fn walk_diagnostic_spans_are_valid() {
        use whisker_types::{RuleId, Severity};

        struct SpanChecker;
        impl LintPass for SpanChecker {
            fn check_node(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                vec![Diagnostic::new(
                    RuleId("test"),
                    Severity::Info,
                    "span check".into(),
                    node.span(),
                )]
            }
        }

        let source = "fn main() { let x = 42; }";
        let tree = parse_rust(source);
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(SpanChecker)];
        let diagnostics = walk(&tree, &mut passes);

        for diag in &diagnostics {
            assert!(diag.span().start() <= diag.span().end());
            assert!(diag.span().end() <= source.len());
        }
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn walk_with_no_passes_always_empty(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let tree = parse_rust(&source);
                let diagnostics = walk(&tree, &mut Vec::new());
                prop_assert!(diagnostics.is_empty());
            }

            #[test]
            fn walk_diagnostic_count_equals_pass_count_times_nodes(
                source in "fn [a-z]+\\(\\) \\{\\}",
                num_passes in 1..=5usize,
            ) {
                struct CountAll;
                impl LintPass for CountAll {
                    fn check_node(
                        &mut self,
                        node: &DecoratedNode<'_>,
                    ) -> Vec<Diagnostic> {
                        vec![Diagnostic::new(
                            whisker_types::RuleId("test"),
                            whisker_types::Severity::Warn,
                            "hit".into(),
                            node.span(),
                        )]
                    }
                }

                let tree = parse_rust(&source);

                let mut single_pass: Vec<Box<dyn LintPass>> =
                    vec![Box::new(CountAll)];
                let single_count = walk(&tree, &mut single_pass).len();

                let mut multi_passes: Vec<Box<dyn LintPass>> = (0..num_passes)
                    .map(|_| Box::new(CountAll) as Box<dyn LintPass>)
                    .collect();
                let multi_count = walk(&tree, &mut multi_passes).len();

                prop_assert_eq!(multi_count, single_count * num_passes);
            }
        }
    }
}
