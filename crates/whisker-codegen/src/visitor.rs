use std::collections::HashSet;

use serde::Deserialize;

/// A node type entry from tree-sitter's `node-types.json`
#[derive(Clone, Debug, Deserialize)]
struct NodeType {
    #[serde(rename = "type")]
    kind: String,
    named: bool,
    #[serde(default)]
    subtypes: Vec<SubtypeRef>,
}

/// A reference to a subtype within a supertype entry
#[derive(Clone, Debug, Deserialize)]
struct SubtypeRef {
    #[serde(rename = "type")]
    kind: String,
    named: bool,
}

/// Generates a language-specific `LintPass` trait and dispatch function
///
/// Reads tree-sitter's `node-types.json` and produces Rust source code
/// containing:
///
/// - A `{Language}LintPass` trait with one `check_{node_kind}` method per
///   named node type, each defaulting to an empty diagnostic list
/// - A `dispatch` function that maps node kind strings to the appropriate
///   typed method, including supertype dispatch
///
/// The `language` parameter controls the trait name prefix (e.g. `"Rust"`
/// produces `RustLintPass`).
///
/// # Errors
///
/// Returns an error if `node_types_json` is not valid JSON or does not
/// match the expected tree-sitter `node-types.json` schema.
///
/// # Examples
///
/// ```
/// let json = r#"[{"type": "source_file", "named": true}]"#;
/// let code = whisker_codegen::generate_visitor(json, "Rust").unwrap();
/// assert!(code.contains("trait RustLintPass"));
/// assert!(code.contains("fn check_source_file"));
/// ```
pub fn generate_visitor(
    node_types_json: &str,
    language: &str,
) -> Result<String, serde_json::Error> {
    let node_types: Vec<NodeType> = serde_json::from_str(node_types_json)?;

    let concrete_nodes: Vec<&NodeType> = node_types
        .iter()
        .filter(|n| n.named && n.subtypes.is_empty())
        .collect();

    let supertypes: Vec<&NodeType> = node_types
        .iter()
        .filter(|n| n.named && !n.subtypes.is_empty())
        .collect();

    let trait_name = format!("{language}LintPass");
    let mut output = String::new();

    output.push_str("use whisker_types::{DecoratedNode, Diagnostic};\n\n");

    output.push_str(&format!(
        "/// Trait for {language} lint rules\n\
         ///\n\
         /// Generated from the {language} tree-sitter grammar's `node-types.json`.\n\
         /// Each method corresponds to a named node type in the grammar and\n\
         /// defaults to returning no diagnostics.\n\
         pub trait {trait_name} {{\n"
    ));

    for node in &concrete_nodes {
        let method = method_name(&node.kind);
        output.push_str(&format!(
            "    /// Checks a `{}` node\n\
             \x20   fn {method}(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {{\n\
             \x20       Vec::new()\n\
             \x20   }}\n\n",
            node.kind
        ));
    }

    for supertype in &supertypes {
        let method = method_name(&supertype.kind);
        output.push_str(&format!(
            "    /// Checks any `{}` node (supertype)\n\
             \x20   fn {method}(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {{\n\
             \x20       Vec::new()\n\
             \x20   }}\n\n",
            supertype.kind
        ));
    }

    output.push_str("}\n\n");

    output.push_str(&format!(
        "/// Dispatches a node to the appropriate method on the lint pass\n\
         ///\n\
         /// Calls the concrete node method and, if the node is a subtype of\n\
         /// any supertype, also calls the supertype method.\n\
         pub fn dispatch(pass: &mut dyn {trait_name}, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {{\n\
         \x20   let mut diagnostics = Vec::new();\n\
         \x20   let kind = node.kind();\n\n\
         \x20   match kind {{\n"
    ));

    for node in &concrete_nodes {
        let method = method_name(&node.kind);
        output.push_str(&format!(
            "        \"{}\" => diagnostics.extend(pass.{method}(node)),\n",
            node.kind
        ));
    }

    output.push_str("        _ => {}\n");
    output.push_str("    }\n\n");

    for supertype in &supertypes {
        let supertype_method = method_name(&supertype.kind);
        let subtype_kinds: Vec<&str> = collect_leaf_subtypes(supertype, &node_types);

        if subtype_kinds.is_empty() {
            continue;
        }

        output.push_str("    match kind {\n");
        for subtype_kind in &subtype_kinds {
            output.push_str(&format!("        \"{subtype_kind}\" |\n"));
        }
        output.truncate(output.len() - 2);
        output.push_str(&format!(
            " => diagnostics.extend(pass.{supertype_method}(node)),\n"
        ));
        output.push_str("        _ => {}\n");
        output.push_str("    }\n\n");
    }

    output.push_str("    diagnostics\n}\n");

    Ok(output)
}

/// Converts a tree-sitter node kind like `function_item` or `_expression`
/// into a method name like `check_function_item` or `check_expression`
fn method_name(kind: &str) -> String {
    let stripped = kind.strip_prefix('_').unwrap_or(kind);
    format!("check_{stripped}")
}

/// Recursively collects all concrete (leaf) subtypes of a supertype
///
/// Supertypes can contain other supertypes as subtypes. This function
/// resolves the transitive closure down to concrete node kinds.
///
/// A leaf reachable by more than one path is returned once, at the position
/// of its first occurrence. Grammars are free to place the same concrete
/// kind under several supertypes, and the generated dispatch arms them with
/// `|`, where a repeat is an `unreachable_pattern` rather than a harmless
/// duplicate.
fn collect_leaf_subtypes<'a>(supertype: &'a NodeType, all_types: &'a [NodeType]) -> Vec<&'a str> {
    let mut leaves = Vec::new();
    for sub_ref in &supertype.subtypes {
        if !sub_ref.named {
            continue;
        }
        let Some(sub_node) = all_types.iter().find(|n| n.kind == sub_ref.kind && n.named) else {
            continue;
        };
        if sub_node.subtypes.is_empty() {
            leaves.push(sub_node.kind.as_str());
        } else {
            leaves.extend(collect_leaf_subtypes(sub_node, all_types));
        }
    }

    let mut seen = HashSet::new();
    leaves.retain(|kind| seen.insert(*kind));
    leaves
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_JSON: &str = r#"[
        {"type": "source_file", "named": true},
        {"type": "function_item", "named": true},
        {"type": ";", "named": false}
    ]"#;

    const SUPERTYPE_JSON: &str = r#"[
        {"type": "source_file", "named": true},
        {"type": "function_item", "named": true},
        {"type": "const_item", "named": true},
        {"type": "_declaration_statement", "named": true, "subtypes": [
            {"type": "function_item", "named": true},
            {"type": "const_item", "named": true}
        ]}
    ]"#;

    const NESTED_SUPERTYPE_JSON: &str = r#"[
        {"type": "integer_literal", "named": true},
        {"type": "string_literal", "named": true},
        {"type": "array_expression", "named": true},
        {"type": "_literal", "named": true, "subtypes": [
            {"type": "integer_literal", "named": true},
            {"type": "string_literal", "named": true}
        ]},
        {"type": "_expression", "named": true, "subtypes": [
            {"type": "_literal", "named": true},
            {"type": "array_expression", "named": true}
        ]}
    ]"#;

    #[test]
    fn generate_visitor_with_minimal_json_produces_trait() {
        let code = generate_visitor(MINIMAL_JSON, "Rust").expect("should parse");

        assert!(code.contains("pub trait RustLintPass {"));
        assert!(code.contains("fn check_source_file("));
        assert!(code.contains("fn check_function_item("));
    }

    #[test]
    fn generate_visitor_excludes_unnamed_nodes() {
        let code = generate_visitor(MINIMAL_JSON, "Rust").expect("should parse");

        assert!(!code.contains("check_semicolon"));
        assert!(!code.contains("\";\""));
    }

    #[test]
    fn generate_visitor_produces_dispatch_function() {
        let code = generate_visitor(MINIMAL_JSON, "Rust").expect("should parse");

        assert!(code.contains("pub fn dispatch("));
        assert!(
            code.contains("\"source_file\" => diagnostics.extend(pass.check_source_file(node))")
        );
    }

    #[test]
    fn generate_visitor_with_supertype_produces_supertype_method() {
        let code = generate_visitor(SUPERTYPE_JSON, "Rust").expect("should parse");

        assert!(code.contains("fn check_declaration_statement("));
    }

    #[test]
    fn generate_visitor_with_supertype_dispatches_subtypes() {
        let code = generate_visitor(SUPERTYPE_JSON, "Rust").expect("should parse");

        assert!(code.contains("\"function_item\""));
        assert!(code.contains("\"const_item\""));
        assert!(code.contains("check_declaration_statement(node)"));
    }

    #[test]
    fn generate_visitor_with_nested_supertypes_resolves_leaves() {
        let code = generate_visitor(NESTED_SUPERTYPE_JSON, "Rust").expect("should parse");

        assert!(code.contains("check_expression("));
        assert!(code.contains("\"integer_literal\""));
        assert!(code.contains("\"string_literal\""));
        assert!(code.contains("\"array_expression\""));
    }

    const DIAMOND_SUPERTYPE_JSON: &str = r#"[
        {"type": "integer_literal", "named": true},
        {"type": "_literal", "named": true, "subtypes": [
            {"type": "integer_literal", "named": true}
        ]},
        {"type": "_primary", "named": true, "subtypes": [
            {"type": "integer_literal", "named": true}
        ]},
        {"type": "_expression", "named": true, "subtypes": [
            {"type": "_literal", "named": true},
            {"type": "_primary", "named": true}
        ]}
    ]"#;

    #[test]
    fn collect_leaf_subtypes_with_diamond_returns_each_leaf_once() {
        let node_types: Vec<NodeType> =
            serde_json::from_str(DIAMOND_SUPERTYPE_JSON).expect("should parse");
        let expression = node_types
            .iter()
            .find(|n| n.kind == "_expression")
            .expect("should have _expression");

        let leaves = collect_leaf_subtypes(expression, &node_types);

        assert_eq!(leaves, vec!["integer_literal"]);
    }

    #[test]
    fn generate_visitor_with_diamond_emits_no_duplicate_arms() {
        let code = generate_visitor(DIAMOND_SUPERTYPE_JSON, "Rust").expect("should parse");

        let dispatch = code
            .split("pub fn dispatch")
            .nth(1)
            .expect("should have dispatch");
        let expression_arm = dispatch
            .split("check_expression(node)")
            .next()
            .expect("should have expression arm");
        let last_match = expression_arm
            .rsplit("match kind {")
            .next()
            .expect("should have match block");

        assert_eq!(last_match.matches("\"integer_literal\"").count(), 1);
    }

    #[test]
    fn generate_visitor_with_different_language_changes_name() {
        let code = generate_visitor(MINIMAL_JSON, "Python").expect("should parse");

        assert!(code.contains("pub trait PythonLintPass {"));
        assert!(!code.contains("RustLintPass"));
    }

    #[test]
    fn generate_visitor_default_impls_return_empty_vec() {
        let code = generate_visitor(MINIMAL_JSON, "Rust").expect("should parse");

        assert!(code.contains("Vec::new()"));
    }

    #[test]
    fn generate_visitor_with_invalid_json_returns_error() {
        let result = generate_visitor("not json", "Rust");
        assert!(result.is_err());
    }

    #[test]
    fn method_name_preserves_internal_underscores() {
        assert_eq!(method_name("match_arm"), "check_match_arm");
    }

    #[test]
    fn method_name_strips_underscore_prefix() {
        assert_eq!(method_name("_expression"), "check_expression");
        assert_eq!(method_name("function_item"), "check_function_item");
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        fn arb_node_type() -> impl Strategy<Value = String> {
            "[a-z][a-z_]{0,20}".prop_map(|s| s)
        }

        fn arb_node_types_json(
            count: impl Into<proptest::collection::SizeRange>,
        ) -> impl Strategy<Value = String> {
            proptest::collection::vec(arb_node_type(), count).prop_map(|types| {
                let entries: Vec<String> = types
                    .iter()
                    .map(|t| format!(r#"{{"type": "{t}", "named": true}}"#))
                    .collect();
                format!("[{}]", entries.join(", "))
            })
        }

        proptest! {
            #[test]
            fn output_contains_trait_name(language in "[A-Z][a-z]{1,10}") {
                let json = r#"[{"type": "x", "named": true}]"#;
                let code = generate_visitor(json, &language).unwrap();

                let expected = format!("pub trait {language}LintPass");
                prop_assert!(code.contains(&expected));
            }

            #[test]
            fn output_contains_dispatch(language in "[A-Z][a-z]{1,10}") {
                let json = r#"[{"type": "x", "named": true}]"#;
                let code = generate_visitor(json, &language).unwrap();

                prop_assert!(code.contains("pub fn dispatch("));
            }

            #[test]
            fn method_count_matches_named_node_count(
                json in arb_node_types_json(1..=20),
            ) {
                let code = generate_visitor(&json, "Test").unwrap();

                let node_types: Vec<NodeType> = serde_json::from_str(&json).unwrap();
                let named_count = node_types
                    .iter()
                    .filter(|n| n.named && n.subtypes.is_empty())
                    .count();

                let check_count = code.matches("fn check_").count();
                prop_assert_eq!(check_count, named_count);
            }

            #[test]
            fn invalid_json_returns_error(input in "\\PC{1,50}") {
                prop_assume!(!input.starts_with('['));
                let result = generate_visitor(&input, "Rust");
                prop_assert!(result.is_err());
            }

            #[test]
            fn method_name_always_starts_with_check(kind in "[a-z_][a-z_]{0,20}") {
                let name = method_name(&kind);
                prop_assert!(name.starts_with("check_"));
            }

            #[test]
            fn method_name_never_has_leading_underscore_after_check(
                kind in "_[a-z][a-z_]{0,20}",
            ) {
                let name = method_name(&kind);
                prop_assert!(!name.starts_with("check__"));
            }

            #[test]
            fn every_check_method_has_default_body(
                json in arb_node_types_json(1..=10),
            ) {
                let code = generate_visitor(&json, "Test").unwrap();
                let check_count = code.matches("fn check_").count();
                let vec_new_count = code.matches("Vec::new()").count();
                prop_assert!(
                    vec_new_count >= check_count,
                    "expected at least {check_count} Vec::new() (one per check method), got {vec_new_count}"
                );
            }
        }
    }
}
