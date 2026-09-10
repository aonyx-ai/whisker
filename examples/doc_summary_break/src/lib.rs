use whisker_rust::RustLintPass;
use whisker_types::{DecoratedNode, Diagnostic, RuleId, Severity};

// 1: Rule struct and documentation
/// Flags a summary line that runs straight into the rest of the comment
pub struct DocSummaryBreak;

// 2: LintPass implementation
impl RustLintPass for DocSummaryBreak {
    fn check_line_comment(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        if !is_doc_line(node) {
            return Vec::new();
        }

        let Some(next_doc_line) = get_next_doc_line(node) else {
            return Vec::new();
        };

        if !is_doc_line(&next_doc_line) || is_blank(&next_doc_line) {
            return Vec::new();
        }

        vec![Diagnostic::new(
            RULE_ID,
            Severity::Warn,
            "the summary line needs a blank `///` line after it".into(),
            node.span(),
        )]
    }
}

// 3: Rule ID
const RULE_ID: RuleId = RuleId::new("lint.doc-summary-break");

impl whisker_rust::DeclaresRules for DocSummaryBreak {
    fn rules(&self) -> Vec<RuleId> {
        vec![RULE_ID]
    }
}

// 4: Lint registration
whisker_rust::export_lints![DocSummaryBreak];

// ... and some helper functions for the lint

/// Returns the line after this one, when this one opens a doc comment
///
/// Answers [`None`] for a line that follows another doc line, because only
/// the first line of a block is a summary. The caller decides whether what
/// comes back is a doc line at all.
fn get_next_doc_line<'a>(node: &DecoratedNode<'a>) -> Option<DecoratedNode<'a>> {
    let Some(parent) = node.parent() else {
        return None;
    };
    let lines = parent.named_children();
    let Some(at) = lines.iter().position(|line| line.id() == node.id()) else {
        return None;
    };

    if at > 0 && is_doc_line(&lines[at - 1]) {
        return None;
    }

    lines.into_iter().nth(at + 1)
}

/// Returns whether the node is a `///` line
fn is_doc_line(node: &DecoratedNode<'_>) -> bool {
    node.kind() == "line_comment" && node.text().starts_with("///")
}

/// Returns whether the line carries nothing but its marker
fn is_blank(node: &DecoratedNode<'_>) -> bool {
    node.text().trim_start_matches('/').trim().is_empty()
}

// <
#[cfg(test)]
mod tests {
    use whisker_rust::RustLintPassAdapter;
    use whisker_testing::{assert_diagnostic, assert_no_diagnostics, execute, parse};
    use whisker_types::{Language, LintPass};

    use super::*;

    fn passes() -> Vec<Box<dyn LintPass>> {
        vec![Box::new(RustLintPassAdapter::new(DocSummaryBreak))]
    }

    /// `////` is a plain comment, not a doc comment, so the summary below it
    /// starts a block and wants a blank line after it
    #[test]
    fn a_summary_under_a_slash_rule_is_flagged() {
        let tree = parse("////\n/// Adds\n/// The rest.\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
    }

    /// `////` is a comment and not a doc comment, so it ends the block rather
    /// than continuing it, and the summary above it stands alone
    #[test]
    fn a_summary_above_a_slash_rule_is_not_flagged() {
        let tree = parse("/// Adds\n////\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DocSummaryBreak>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DocSummaryBreak>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DocSummaryBreak>();
    }

    #[test]
    fn a_blank_line_after_the_summary_is_not_flagged() {
        let tree = parse("/// Adds\n///\n/// The rest.\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn a_lone_summary_is_not_flagged() {
        let tree = parse("/// Adds\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn a_plain_comment_is_not_flagged() {
        let tree = parse("// Adds\n// The rest.\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn a_second_line_against_the_summary_is_flagged() {
        let tree = parse("/// Adds\n/// The rest.\nfn f() {}", Language::Rust);

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0]).has_rule_id("lint.doc-summary-break");
    }

    #[test]
    fn only_the_first_line_of_a_block_is_flagged() {
        let tree = parse(
            "/// Adds\n/// The rest.\n/// And more.\nfn f() {}",
            Language::Rust,
        );

        let diagnostics = execute(&tree, &mut passes());

        assert_eq!(diagnostics.len(), 1);
    }
}
// >
