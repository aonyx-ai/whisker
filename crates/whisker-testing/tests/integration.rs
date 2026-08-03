use whisker_rust::{RustLintPass, RustLintPassAdapter};
use whisker_testing::{assert_diagnostic, assert_no_diagnostics, execute, parse};
use whisker_types::{DecoratedNode, Diagnostic, Language, LintPass, RuleId, Severity};

fn adapt<P: RustLintPass + Send + Sync + 'static>(pass: P) -> Box<dyn LintPass> {
    Box::new(RustLintPassAdapter::new(pass))
}

struct MatchArmWildcardFinder;

impl RustLintPass for MatchArmWildcardFinder {
    fn check_match_arm(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let text = node.text();
        if text.starts_with('_') && !text.starts_with("__") {
            vec![Diagnostic::new(
                RuleId("lint.wildcard-match-arm"),
                Severity::Warn,
                "wildcard match arm".into(),
                node.span(),
            )]
        } else {
            Vec::new()
        }
    }
}

#[test]
fn wildcard_finder_detects_wildcard_arm() {
    let tree = parse("fn f() { match x { 1 => {} _ => {} } }", Language::Rust);
    let mut passes = vec![adapt(MatchArmWildcardFinder)];
    let diagnostics = execute(&tree, &mut passes);

    assert_eq!(diagnostics.len(), 1);
    assert_diagnostic(&diagnostics[0])
        .has_rule_id("lint.wildcard-match-arm")
        .has_severity(Severity::Warn)
        .message_contains("wildcard");
}

#[test]
fn wildcard_finder_ignores_named_arms() {
    let tree = parse("fn f() { match x { Foo => {} Bar => {} } }", Language::Rust);
    let mut passes = vec![adapt(MatchArmWildcardFinder)];
    let diagnostics = execute(&tree, &mut passes);

    assert_no_diagnostics(&diagnostics);
}

struct MacroInvocationFinder;

impl RustLintPass for MacroInvocationFinder {
    fn check_macro_invocation(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let text = node.text();
        if text.starts_with("matches!") {
            vec![Diagnostic::new(
                RuleId("lint.no-matches-macro"),
                Severity::Warn,
                "use of matches! macro".into(),
                node.span(),
            )]
        } else {
            Vec::new()
        }
    }
}

#[test]
fn macro_finder_detects_matches_macro() {
    let tree = parse("fn f() { let _ = matches!(x, 1); }", Language::Rust);
    let mut passes = vec![adapt(MacroInvocationFinder)];
    let diagnostics = execute(&tree, &mut passes);

    assert_eq!(diagnostics.len(), 1);
    assert_diagnostic(&diagnostics[0])
        .has_rule_id("lint.no-matches-macro")
        .message_contains("matches!");
}

#[test]
fn macro_finder_ignores_println() {
    let tree = parse("fn f() { println!(\"hello\"); }", Language::Rust);
    let mut passes = vec![adapt(MacroInvocationFinder)];
    let diagnostics = execute(&tree, &mut passes);

    assert_no_diagnostics(&diagnostics);
}

struct BoolParamFinder;

impl RustLintPass for BoolParamFinder {
    fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let text = node.text();
        if text.contains(": bool") {
            vec![Diagnostic::new(
                RuleId("lint.bool-param"),
                Severity::Warn,
                "bool parameter".into(),
                node.span(),
            )]
        } else {
            Vec::new()
        }
    }
}

#[test]
fn bool_finder_detects_bool_param() {
    let tree = parse("fn f(x: bool) {}", Language::Rust);
    let mut passes = vec![adapt(BoolParamFinder)];
    let diagnostics = execute(&tree, &mut passes);

    assert_eq!(diagnostics.len(), 1);
    assert_diagnostic(&diagnostics[0])
        .has_rule_id("lint.bool-param")
        .message_contains("bool");
}

#[test]
fn bool_finder_ignores_non_bool_params() {
    let tree = parse("fn f(x: i32) {}", Language::Rust);
    let mut passes = vec![adapt(BoolParamFinder)];
    let diagnostics = execute(&tree, &mut passes);

    assert_no_diagnostics(&diagnostics);
}

#[test]
fn multiple_lint_passes_run_together() {
    let tree = parse(
        "fn f(x: bool) { let _ = matches!(x, true); }",
        Language::Rust,
    );
    let mut passes = vec![adapt(BoolParamFinder), adapt(MacroInvocationFinder)];
    let diagnostics = execute(&tree, &mut passes);

    assert_eq!(diagnostics.len(), 2);

    let rule_ids: Vec<&str> = diagnostics.iter().map(|d| d.rule_id().as_str()).collect();
    assert!(rule_ids.contains(&"lint.bool-param"));
    assert!(rule_ids.contains(&"lint.no-matches-macro"));
}

#[test]
fn diagnostic_spans_point_to_correct_byte_ranges() {
    let source = "fn main() { let _ = matches!(x, 1); }";
    let tree = parse(source, Language::Rust);
    let mut passes = vec![adapt(MacroInvocationFinder)];
    let diagnostics = execute(&tree, &mut passes);

    assert_eq!(diagnostics.len(), 1);
    let span = diagnostics[0].span();
    let spanned_text = &source[span.start()..span.end()];
    assert!(
        spanned_text.contains("matches!"),
        "span should cover the matches! invocation, got: {spanned_text}"
    );
}

#[test]
fn empty_source_produces_no_diagnostics() {
    let tree = parse("", Language::Rust);
    let mut passes = vec![
        adapt(MatchArmWildcardFinder),
        adapt(MacroInvocationFinder),
        adapt(BoolParamFinder),
    ];
    let diagnostics = execute(&tree, &mut passes);
    assert_no_diagnostics(&diagnostics);
}

#[test]
fn complex_source_does_not_panic() {
    let source = r#"
        use std::collections::HashMap;

        #[derive(Debug, Clone)]
        struct Config {
            name: String,
            values: HashMap<String, i32>,
        }

        impl Config {
            fn new(name: &str) -> Self {
                Self {
                    name: name.to_string(),
                    values: HashMap::new(),
                }
            }

            fn get(&self, key: &str) -> Option<&i32> {
                self.values.get(key)
            }
        }

        fn process(configs: &[Config]) -> Vec<String> {
            configs
                .iter()
                .filter(|c| !c.name.is_empty())
                .map(|c| c.name.clone())
                .collect()
        }
    "#;

    let tree = parse(source, Language::Rust);
    let mut passes = vec![
        adapt(MatchArmWildcardFinder),
        adapt(MacroInvocationFinder),
        adapt(BoolParamFinder),
    ];
    let diagnostics = execute(&tree, &mut passes);
    assert_no_diagnostics(&diagnostics);
}
