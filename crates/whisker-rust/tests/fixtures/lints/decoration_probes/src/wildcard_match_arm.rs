use whisker_rust::decorations::{AdtFlags, ResolvedType};
use whisker_rust::{RustLintPass, RustLintPassAdapter};
use whisker_types::{DecoratedNode, Diagnostic, LintPass, RuleId, Severity};

/// The rule id this probe reports under
const RULE_ID: RuleId = RuleId::new("fixture.wildcard-match-arm");

/// Flags a `_` arm when the scrutinee resolves to an enum
///
/// The decoration under test is [`ResolvedType`] on the scrutinee, which
/// the provider has to resolve through whatever expression produced it: a
/// bare name, a field access, or a call. [`AdtFlags`] answers the second
/// question, whether the enum is one this crate may exhaust.
pub struct WildcardMatchArm;

impl WildcardMatchArm {
    /// Creates a boxed [`LintPass`] suitable for the whisker pipeline
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pass = WildcardMatchArm::into_lint_pass();
    /// ```
    pub fn into_lint_pass() -> Box<dyn LintPass> {
        Box::new(RustLintPassAdapter::new(Self))
    }
}

impl RustLintPass for WildcardMatchArm {
    fn check_match_expression(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        let Some(scrutinee) = node.child_by_field_name("value") else {
            return Vec::new();
        };

        let Some(resolved) = scrutinee.decoration::<ResolvedType>() else {
            return Vec::new();
        };
        if !resolved.is_enum() {
            return Vec::new();
        }

        if let Some(flags) = scrutinee.decoration::<AdtFlags>()
            && flags.non_exhaustive_external()
        {
            return Vec::new();
        }

        let Some(body) = node.child_by_field_name("body") else {
            return Vec::new();
        };

        let mut diagnostics = Vec::new();

        for arm in body.named_children() {
            if arm.kind() != "match_arm" {
                continue;
            }
            let Some(pattern) = arm.child_by_field_name("pattern") else {
                continue;
            };
            if is_wildcard_pattern(&pattern) {
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Warn,
                    "wildcard match arm hides unhandled variants".into(),
                    pattern.span(),
                ));
            }
        }

        diagnostics
    }
}

/// Returns whether a match pattern node represents a wildcard `_`
///
/// Tree-sitter emits the `_` token as an anonymous node, so the search
/// reads every child rather than only the named ones.
fn is_wildcard_pattern(match_pattern: &DecoratedNode<'_>) -> bool {
    for index in 0..match_pattern.child_count() as u32 {
        let Some(child) = match_pattern.child(index) else {
            continue;
        };
        if !child.is_named() && child.text() == "_" {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use whisker_testing::{assert_no_diagnostics, decorate, execute, parse};
    use whisker_types::{DecoratedTree, DecorationMap, Language};

    use super::*;

    /// Returns the id of the first match expression's scrutinee
    fn find_scrutinee_id(tree: &DecoratedTree) -> usize {
        fn walk(node: &DecoratedNode<'_>) -> Option<usize> {
            if node.kind() == "match_expression" {
                return node
                    .child_by_field_name("value")
                    .map(|scrutinee| scrutinee.id());
            }
            for child in node.named_children() {
                if let Some(id) = walk(&child) {
                    return Some(id);
                }
            }
            None
        }

        walk(&tree.root_node()).expect("source should hold a match expression")
    }

    /// Runs the probe with `resolved` attached to the scrutinee
    fn run(source: &str, resolved: ResolvedType, flags: Option<AdtFlags>) -> Vec<Diagnostic> {
        let mut tree = parse(source, Language::Rust);
        let scrutinee = find_scrutinee_id(&tree);
        let mut map = DecorationMap::new();
        map.insert(scrutinee, resolved);
        if let Some(flags) = flags {
            map.insert(scrutinee, flags);
        }
        decorate(&mut tree, map);

        execute(&tree, &mut vec![WildcardMatchArm::into_lint_pass()])
    }

    const SOURCE: &str = "fn f(x: Color) { match x { Color::Red => {} _ => {} } }";

    #[test]
    fn wildcard_on_an_enum_scrutinee_is_flagged() {
        let diagnostics = run(
            SOURCE,
            ResolvedType::new("Color".into()).with_enum(true),
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].rule_id().as_str(),
            "fixture.wildcard-match-arm"
        );
    }

    #[test]
    fn wildcard_on_a_non_enum_scrutinee_is_not_flagged() {
        let diagnostics = run(SOURCE, ResolvedType::new("u32".into()), None);

        assert_no_diagnostics(&diagnostics);
    }

    /// Pins the exemption an [`AdtFlags`] decoration grants
    ///
    /// A `_` arm over an enum another crate marked `#[non_exhaustive]` is
    /// the one wildcard that has to stay: the crate can add a variant
    /// without a breaking change, so exhausting the enum here would not
    /// compile tomorrow. Only the provider can tell that enum from one
    /// this crate may exhaust, which is what this pins.
    #[test]
    fn wildcard_on_an_externally_non_exhaustive_enum_is_not_flagged() {
        let diagnostics = run(
            SOURCE,
            ResolvedType::new("Color".into()).with_enum(true),
            Some(AdtFlags::new(true)),
        );

        assert_no_diagnostics(&diagnostics);
    }

    /// Pins the other answer the same decoration can give
    ///
    /// An enum this crate may exhaust keeps its diagnostic, so the
    /// exemption above is read from the decoration rather than granted to
    /// every enum that carries one.
    #[test]
    fn wildcard_on_an_enum_this_crate_may_exhaust_is_flagged() {
        let diagnostics = run(
            SOURCE,
            ResolvedType::new("Color".into()).with_enum(true),
            Some(AdtFlags::new(false)),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].rule_id().as_str(),
            "fixture.wildcard-match-arm"
        );
    }

    #[test]
    fn wildcard_on_an_undecorated_scrutinee_is_not_flagged() {
        let tree = parse(SOURCE, Language::Rust);

        let diagnostics = execute(&tree, &mut vec![WildcardMatchArm::into_lint_pass()]);

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WildcardMatchArm>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<WildcardMatchArm>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<WildcardMatchArm>();
    }
}
