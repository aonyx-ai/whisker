use whisker_rust::decorations::ImportSource;
use whisker_rust::{RustLintPass, RustLintPassAdapter};
use whisker_types::{DecoratedNode, Diagnostic, LintPass, RuleId, Severity};

/// The rule id this probe reports under
const RULE_ID: RuleId = RuleId::new("fixture.function-scoped-import");

/// Flags a `use` in a function body unless its qualifier is an enum
///
/// The decoration under test is [`ImportSource`]. Whether a qualifier
/// names an enum, a module, or something the provider could not resolve
/// is not a syntactic question: `Color::Red` and `Shapes::draw` are
/// spelled the same way and only one of them imports variants. The probe
/// takes the provider's word for it and reports what that answer implies.
pub struct FunctionScopedImport;

impl FunctionScopedImport {
    /// Creates a boxed [`LintPass`] suitable for the whisker pipeline
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pass = FunctionScopedImport::into_lint_pass();
    /// ```
    pub fn into_lint_pass() -> Box<dyn LintPass> {
        Box::new(RustLintPassAdapter::new(Self))
    }
}

impl RustLintPass for FunctionScopedImport {
    fn check_use_declaration(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        if !is_inside_block(node) {
            return Vec::new();
        }

        if imports_enum_variants(node) {
            return Vec::new();
        }

        vec![Diagnostic::new(
            RULE_ID,
            Severity::Warn,
            "`use` sits inside a function body; move it to the top of the module".into(),
            node.span(),
        )]
    }
}

/// Returns whether the node sits in a block rather than at module level
///
/// The walk stops at the nearest scope, so a `use` at the top of a module
/// nested in a function counts as module level.
fn is_inside_block(node: &DecoratedNode<'_>) -> bool {
    let mut current = node.parent();

    loop {
        let Some(ancestor) = current else {
            return false;
        };
        match ancestor.kind() {
            "block" => return true,
            "declaration_list" | "source_file" => return false,
            _ => current = ancestor.parent(),
        }
    }
}

/// Returns whether the import names variants of an enum
///
/// Only the provider's resolution of the qualifier answers this; a
/// capitalized qualifier proves nothing on its own. An undecorated node
/// therefore keeps the diagnostic, because the exemption needs proof.
fn imports_enum_variants(node: &DecoratedNode<'_>) -> bool {
    match node.decoration::<ImportSource>() {
        Some(ImportSource::Enum) => true,
        Some(ImportSource::Module) => false,
        Some(ImportSource::Other) => false,
        Some(ImportSource::Unresolved) => false,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use whisker_testing::{assert_no_diagnostics, decorate, execute, parse};
    use whisker_types::{DecorationMap, Language};

    use super::*;

    /// Runs the probe over `source` with no decorations at all
    fn run(source: &str) -> Vec<Diagnostic> {
        let tree = parse(source, Language::Rust);

        execute(&tree, &mut vec![FunctionScopedImport::into_lint_pass()])
    }

    /// Runs the probe with `import_source` on every `use` in `source`
    fn run_with(source: &str, import_source: ImportSource) -> Vec<Diagnostic> {
        fn collect(node: &DecoratedNode<'_>, ids: &mut Vec<usize>) {
            if node.kind() == "use_declaration" {
                ids.push(node.id());
            }
            for child in node.named_children() {
                collect(&child, ids);
            }
        }

        let mut tree = parse(source, Language::Rust);
        let mut ids = Vec::new();
        collect(&tree.root_node(), &mut ids);
        let mut map = DecorationMap::new();
        for id in ids {
            map.insert(id, import_source);
        }
        decorate(&mut tree, map);

        execute(&tree, &mut vec![FunctionScopedImport::into_lint_pass()])
    }

    #[test]
    fn import_in_a_function_body_is_flagged() {
        let diagnostics = run("fn f() { use std::collections::HashMap; }");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].rule_id().as_str(),
            "fixture.function-scoped-import"
        );
    }

    #[test]
    fn import_at_module_level_is_not_flagged() {
        let diagnostics = run("use std::collections::HashMap;\n\nfn f() {}");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn import_in_a_nested_module_is_not_flagged() {
        let diagnostics = run("fn f() { mod inner { use std::fmt; } }");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn import_of_enum_variants_is_not_flagged() {
        let diagnostics = run_with("fn f() { use Color::Red; }", ImportSource::Enum);

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn import_of_a_module_is_flagged() {
        let diagnostics = run_with("fn f() { use Shapes::draw; }", ImportSource::Module);

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FunctionScopedImport>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FunctionScopedImport>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FunctionScopedImport>();
    }
}
