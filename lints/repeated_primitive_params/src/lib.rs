use whisker_rust::{RustLintPass, RustLintPassAdapter};
use whisker_types::{DecoratedNode, Diagnostic, LintPass, RuleId, Severity};

const RULE_ID: RuleId = RuleId::new("lint.repeated-primitive-params");

/// Flags a function that takes two or more parameters of one primitive type
///
/// Each parameter accepts the argument meant for the other, so a caller can
/// swap them and the code still compiles. A newtype for each parameter turns
/// the swap into a type error.
///
/// A lone primitive parameter is safe, because it has no sibling to swap
/// with. Return types and struct fields are not argument positions, so the
/// rule never looks at them. `bool` belongs to `lint.bool-param`.
pub struct RepeatedPrimitiveParams;

impl RepeatedPrimitiveParams {
    /// Creates a boxed [`LintPass`] suitable for the whisker pipeline
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pass = RepeatedPrimitiveParams::into_lint_pass();
    /// ```
    pub fn into_lint_pass() -> Box<dyn LintPass> {
        Box::new(RustLintPassAdapter::new(Self))
    }
}

/// The parameters of one signature grouped under one primitive type
struct ParameterGroup<'a> {
    type_name: String,
    type_node: DecoratedNode<'a>,
    parameter_names: Vec<&'a str>,
}

/// Returns the name to group a parameter under, or [`None`] when the rule
/// does not track the type
///
/// The rule tracks the primitive types and `String`, but not `bool`, which
/// `lint.bool-param` owns. A reference keeps its `&` and its `mut`: a
/// `String` does not fit a `&str` parameter, and a `&str` does not fit a
/// `&mut str` one. A lifetime is not part of that decision, so it drops out
/// and `&'a str` groups with `&str`.
fn primitive_type_name(node: &DecoratedNode<'_>) -> Option<String> {
    match node.kind() {
        "primitive_type" => match node.text() {
            "bool" => None,
            text => Some(text.to_string()),
        },
        "type_identifier" => match node.text() {
            "String" => Some("String".to_string()),
            _ => None,
        },
        "reference_type" => {
            let inner = node.child_by_field_name("type")?;
            let inner = primitive_type_name(&inner)?;
            let mutable = node
                .named_children()
                .iter()
                .any(|child| child.kind() == "mutable_specifier");
            match mutable {
                true => Some(format!("&mut {inner}")),
                false => Some(format!("&{inner}")),
            }
        }
        _ => None,
    }
}

/// Returns whether the signature crosses a foreign ABI boundary
///
/// A function in an `extern` block, or one with an `extern` ABI, must match a
/// signature that a foreign caller fixes. Its parameter types are not a free
/// choice.
fn is_foreign(node: &DecoratedNode<'_>) -> bool {
    let extern_modifier = node.named_children().iter().any(|child| {
        child.kind() == "function_modifiers"
            && child
                .named_children()
                .iter()
                .any(|modifier| modifier.kind() == "extern_modifier")
    });
    let foreign_block = node
        .parent()
        .and_then(|parent| parent.parent())
        .is_some_and(|grandparent| grandparent.kind() == "foreign_mod_item");

    extern_modifier || foreign_block
}

/// Joins parameter names into an English list, each name in backticks
fn join_names(names: &[&str]) -> String {
    let names: Vec<String> = names.iter().map(|name| format!("`{name}`")).collect();
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, [first])) => format!("{first} and {last}"),
        Some((last, rest)) => format!("{}, and {last}", rest.join(", ")),
    }
}

/// Reports every primitive type that the signature uses more than once
///
/// The rule skips a foreign signature. The diagnostic points at the type of
/// the first parameter in the group, so two repeated types give two
/// diagnostics at two places.
fn check_signature(node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
    if is_foreign(node) {
        return Vec::new();
    }

    let Some(parameters) = node.child_by_field_name("parameters") else {
        return Vec::new();
    };

    let mut groups: Vec<ParameterGroup<'_>> = Vec::new();
    for parameter in parameters.named_children() {
        if parameter.kind() != "parameter" {
            continue;
        }
        let Some(type_node) = parameter.child_by_field_name("type") else {
            continue;
        };
        let Some(type_name) = primitive_type_name(&type_node) else {
            continue;
        };
        let Some(pattern) = parameter.child_by_field_name("pattern") else {
            continue;
        };

        match groups.iter_mut().find(|group| group.type_name == type_name) {
            Some(group) => group.parameter_names.push(pattern.text()),
            None => groups.push(ParameterGroup {
                type_name,
                type_node,
                parameter_names: vec![pattern.text()],
            }),
        }
    }

    let mut diagnostics = Vec::new();
    for ParameterGroup {
        type_name,
        type_node,
        parameter_names,
    } in groups
    {
        if parameter_names.len() < 2 {
            continue;
        }
        diagnostics.push(Diagnostic::new(
            RULE_ID,
            Severity::Warn,
            format!(
                "parameters {} share type `{type_name}`; a caller can transpose them, \
                 so give each one a newtype",
                join_names(&parameter_names)
            ),
            type_node.span(),
        ));
    }
    diagnostics
}

impl RustLintPass for RepeatedPrimitiveParams {
    fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        check_signature(node)
    }

    fn check_function_signature_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        check_signature(node)
    }
}

#[cfg(feature = "plugin")]
whisker_rust::export_lints![RepeatedPrimitiveParams];

#[cfg(test)]
mod tests {
    use whisker_testing::{assert_diagnostic, assert_no_diagnostics, execute, parse};
    use whisker_types::{Language, Severity};

    use super::*;

    fn run(source: &str) -> Vec<Diagnostic> {
        let tree = parse(source, Language::Rust);
        let mut passes = vec![RepeatedPrimitiveParams::into_lint_pass()];
        execute(&tree, &mut passes)
    }

    #[test]
    fn check_signature_with_bool_pair_reports_nothing() {
        let diagnostics = run("fn f(a: bool, b: bool) {}");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn check_signature_with_extern_function_reports_nothing() {
        let diagnostics = run("pub extern \"C\" fn f(a: usize, b: usize) {}");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn check_signature_with_foreign_block_reports_nothing() {
        let diagnostics = run("unsafe extern \"C\" { fn f(a: usize, b: usize); }");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn check_signature_with_mixed_reference_and_owned_reports_nothing() {
        let diagnostics = run("fn f(a: &str, b: String) {}");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn check_signature_with_mutable_and_shared_reference_reports_nothing() {
        let diagnostics = run("fn f(a: &usize, b: &mut usize) {}");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn check_signature_with_named_types_reports_nothing() {
        let diagnostics = run("fn f(a: UserId, b: Email) {}");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn check_signature_with_repeated_lifetime_reference_reports_group() {
        let diagnostics = run("fn f<'a>(a: &'a str, b: &'a str) {}");

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0]).message_contains("share type `&str`");
    }

    #[test]
    fn check_signature_with_repeated_mutable_reference_reports_group() {
        let diagnostics = run("fn f(functions: &mut usize, signatures: &mut usize) {}");

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .message_contains("parameters `functions` and `signatures` share type `&mut usize`");
    }

    #[test]
    fn check_signature_with_repeated_string_reports_group() {
        let source = "fn send_email(to: String, from: String) {}";

        let diagnostics = run(source);

        let start = source.find("String").expect("source contains a type");
        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .has_rule_id("lint.repeated-primitive-params")
            .has_severity(Severity::Warn)
            .message_contains("parameters `to` and `from` share type `String`")
            .has_span("<test>", start, start + "String".len());
    }

    #[test]
    fn check_signature_with_repeated_type_on_method_reports_group() {
        let diagnostics = run("impl S { fn f(&self, a: usize, b: usize) {} }");

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0]).message_contains("parameters `a` and `b`");
    }

    #[test]
    fn check_signature_with_repeated_type_on_trait_method_reports_group() {
        let diagnostics = run("trait T { fn f(&self, a: usize, b: usize); }");

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0]).message_contains("parameters `a` and `b`");
    }

    #[test]
    fn check_signature_with_single_primitive_reports_nothing() {
        let diagnostics = run("fn f(a: String, b: usize, c: Config) {}");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn check_signature_with_two_repeated_types_reports_each_group() {
        let diagnostics = run("fn f(a: usize, b: String, c: usize, d: String) {}");

        assert_eq!(diagnostics.len(), 2);
        assert_diagnostic(&diagnostics[0])
            .message_contains("parameters `a` and `c` share type `usize`");
        assert_diagnostic(&diagnostics[1])
            .message_contains("parameters `b` and `d` share type `String`");
    }

    #[test]
    fn check_signature_with_unsafe_modifier_reports_group() {
        let diagnostics = run("unsafe fn f(a: usize, b: usize) {}");

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0]).message_contains("share type `usize`");
    }

    #[test]
    fn join_names_with_one_name_returns_that_name() {
        let joined = join_names(&["a"]);

        assert_eq!(joined, "`a`");
    }

    #[test]
    fn join_names_with_three_names_returns_oxford_comma_list() {
        let joined = join_names(&["a", "b", "c"]);

        assert_eq!(joined, "`a`, `b`, and `c`");
    }

    #[test]
    fn join_names_with_two_names_returns_pair() {
        let joined = join_names(&["a", "b"]);

        assert_eq!(joined, "`a` and `b`");
    }

    #[test]
    fn join_names_with_zero_names_returns_empty() {
        let joined = join_names(&[]);

        assert_eq!(joined, "");
    }

    #[test]
    fn primitive_type_name_with_nested_reference_keeps_both_markers() {
        let diagnostics = run("fn f(a: &&str, b: &&str) {}");

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0]).message_contains("share type `&&str`");
    }

    #[test]
    fn primitive_type_name_with_three_repeated_params_lists_all_three() {
        let diagnostics = run("fn f(a: u32, b: u32, c: u32) {}");

        assert_eq!(diagnostics.len(), 1);
        assert_diagnostic(&diagnostics[0])
            .message_contains("parameters `a`, `b`, and `c` share type `u32`");
    }

    #[test]
    fn primitive_type_name_with_wrapped_primitive_reports_nothing() {
        let diagnostics = run("fn f(a: Option<usize>, b: Option<usize>) {}");

        assert_no_diagnostics(&diagnostics);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RepeatedPrimitiveParams>();
        assert_send::<ParameterGroup<'_>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RepeatedPrimitiveParams>();
        assert_sync::<ParameterGroup<'_>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<RepeatedPrimitiveParams>();
        assert_unpin::<ParameterGroup<'_>>();
    }
}
