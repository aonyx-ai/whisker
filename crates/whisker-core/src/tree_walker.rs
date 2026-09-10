use whisker_types::{DecoratedNode, DecoratedTree, Diagnostic, LintPass, Panic};

use crate::PassPanic;

/// Walks a decorated syntax tree and collects diagnostics from lint passes
///
/// Performs a depth-first traversal of all named nodes, calling each lint
/// pass for every node visited.
///
/// # Errors
///
/// Returns a [`PassPanic`] when a pass panics, naming the node it was
/// checking. The walk stops there, for the reason that type gives.
pub fn walk(
    tree: &DecoratedTree,
    passes: &mut [Box<dyn LintPass>],
) -> Result<Vec<Diagnostic>, PassPanic> {
    let mut diagnostics = Vec::new();
    visit_node(&tree.root_node(), passes, &mut diagnostics)?;
    Ok(diagnostics)
}

fn visit_node(
    node: &DecoratedNode<'_>,
    passes: &mut [Box<dyn LintPass>],
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), PassPanic> {
    if node.is_named() {
        for pass in passes.iter_mut() {
            let checked: Result<_, Panic> = pass.check_node(node).into();
            let found = checked.map_err(|panic| PassPanic::new(node.kind(), node.span(), panic))?;
            diagnostics.extend(found);
        }
    }

    for child in node.named_children() {
        visit_node(&child, passes, diagnostics)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use stabby::vec;
    use whisker_types::{Checked, Configured, DecoratedTree, RuleOptions};

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
        let diagnostics = walk(&tree, &mut Vec::new()).expect("should walk");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn walk_visits_all_named_nodes() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Counter;
        impl LintPass for Counter {
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                COUNT.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        COUNT.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn main() {}");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(Counter)];
        walk(&tree, &mut passes).expect("should walk");

        assert!(COUNT.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn walk_on_empty_source_returns_empty() {
        let tree = parse_rust("");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(CounterPass(0))];
        let diagnostics = walk(&tree, &mut passes).expect("should walk");
        assert!(diagnostics.is_empty());
    }

    struct CounterPass(usize);
    impl LintPass for CounterPass {
        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
            self.0 += 1;
            Checked::Ok(vec::Vec::new())
        }
    }

    #[test]
    fn walk_visits_nested_nodes() {
        static NESTED_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct KindCounter;
        impl LintPass for KindCounter {
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                NESTED_COUNT.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        NESTED_COUNT.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn main() { let x = 1; }");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(KindCounter)];
        walk(&tree, &mut passes).expect("should walk");

        let count = NESTED_COUNT.load(Ordering::Relaxed);
        assert!(count > 1, "should visit multiple nested nodes, got {count}");
    }

    #[test]
    fn walk_visits_deeply_nested_code() {
        static DEEP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct DepthCounter;
        impl LintPass for DepthCounter {
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                DEEP_COUNT.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        DEEP_COUNT.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn f() { if true { if true { if true { let x = 1; } } } }");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(DepthCounter)];
        walk(&tree, &mut passes).expect("should walk");

        let count = DEEP_COUNT.load(Ordering::Relaxed);
        assert!(
            count >= 4,
            "deeply nested source should visit at least 4 nodes, got {count}"
        );
    }

    #[test]
    fn walk_multiple_passes_each_see_same_nodes() {
        static PASS_A: AtomicUsize = AtomicUsize::new(0);
        static PASS_B: AtomicUsize = AtomicUsize::new(0);

        struct CounterA;
        impl LintPass for CounterA {
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                PASS_A.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        struct CounterB;
        impl LintPass for CounterB {
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                PASS_B.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        PASS_A.store(0, Ordering::Relaxed);
        PASS_B.store(0, Ordering::Relaxed);

        let tree = parse_rust("fn a() {} fn b() {}");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(CounterA), Box::new(CounterB)];
        walk(&tree, &mut passes).expect("should walk");

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
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                let found = match node.kind() == "function_item" {
                    true => vec![Diagnostic::new(
                        RuleId::new(self.0),
                        Severity::Warn,
                        format!("{} found fn", self.0),
                        node.span(),
                    )],
                    false => Vec::new(),
                };
                Checked::Ok(found.into_iter().collect())
            }
        }

        let tree = parse_rust("fn main() {}");
        let mut passes: Vec<Box<dyn LintPass>> =
            vec![Box::new(WarnOnFn("pass.a")), Box::new(WarnOnFn("pass.b"))];
        let diagnostics = walk(&tree, &mut passes).expect("should walk");

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].rule_id(), RuleId::new("pass.a"));
        assert_eq!(diagnostics[1].rule_id(), RuleId::new("pass.b"));
    }

    #[test]
    fn walk_diagnostic_spans_are_valid() {
        use whisker_types::{RuleId, Severity};

        struct SpanChecker;
        impl LintPass for SpanChecker {
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                Checked::Ok(
                    [Diagnostic::new(
                        RuleId::new("test"),
                        Severity::Info,
                        "span check".into(),
                        node.span(),
                    )]
                    .into_iter()
                    .collect(),
                )
            }
        }

        let source = "fn main() { let x = 42; }";
        let tree = parse_rust(source);
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(SpanChecker)];
        let diagnostics = walk(&tree, &mut passes).expect("should walk");

        for diag in &diagnostics {
            assert!(diag.span().start() <= diag.span().end());
            assert!(diag.span().end() <= source.len());
        }
    }

    /// A panic stops the walk where it happened and says where that was
    ///
    /// Nodes after the panic go unchecked on purpose: the pass is in a
    /// state its author never meant.
    #[test]
    fn walk_with_a_panicking_pass_stops_and_names_the_node() {
        static AFTER_PANIC: AtomicUsize = AtomicUsize::new(0);

        struct PanicOnFn;
        impl LintPass for PanicOnFn {
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked {
                let kind = node.kind();
                Panic::catch(move || -> Vec<Diagnostic> {
                    match kind == "function_item" {
                        true => panic!("no functions"),
                        false => Vec::new(),
                    }
                })
                .map(|found| found.into_iter().collect())
                .into()
            }
        }

        struct CountAfter;
        impl LintPass for CountAfter {
            extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                Configured::Ok(())
            }

            extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
                AFTER_PANIC.fetch_add(1, Ordering::Relaxed);
                Checked::Ok(vec::Vec::new())
            }
        }

        AFTER_PANIC.store(0, Ordering::Relaxed);
        let tree = parse_rust("fn main() { let x = 1; }");
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(PanicOnFn), Box::new(CountAfter)];

        let error = walk(&tree, &mut passes).expect_err("the panic should end the walk");

        assert_eq!(error.kind(), "function_item");
        assert_eq!(error.span().start(), 0);
        assert_eq!(error.panic().message(), "no functions");
        assert_eq!(AFTER_PANIC.load(Ordering::Relaxed), 1);
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
                let diagnostics = walk(&tree, &mut Vec::new()).expect("should walk");
                prop_assert!(diagnostics.is_empty());
            }

            #[test]
            fn walk_diagnostic_count_equals_pass_count_times_nodes(
                source in "fn [a-z]+\\(\\) \\{\\}",
                num_passes in 1..=5usize,
            ) {
                struct CountAll;
                impl LintPass for CountAll {
                    extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
                        Configured::Ok(())
                    }

                    extern "C" fn check_node(
                        &mut self,
                        node: &DecoratedNode<'_>,
                    ) -> Checked {
                        Checked::Ok(
                            [Diagnostic::new(
                                whisker_types::RuleId::new("test"),
                                whisker_types::Severity::Warn,
                                "hit".into(),
                                node.span(),
                            )]
                            .into_iter()
                            .collect(),
                        )
                    }
                }

                let tree = parse_rust(&source);

                let mut single_pass: Vec<Box<dyn LintPass>> =
                    vec![Box::new(CountAll)];
                let single_count = walk(&tree, &mut single_pass).expect("should walk").len();

                let mut multi_passes: Vec<Box<dyn LintPass>> = (0..num_passes)
                    .map(|_| Box::new(CountAll) as Box<dyn LintPass>)
                    .collect();
                let multi_count = walk(&tree, &mut multi_passes).expect("should walk").len();

                prop_assert_eq!(multi_count, single_count * num_passes);
            }
        }
    }
}
