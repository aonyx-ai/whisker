use serde::{Deserialize, Serialize};

/// One entry of `node-types.json`, reduced to what the visitor reads
#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
struct NodeType {
    #[serde(rename = "type")]
    kind: String,
    named: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    subtypes: Vec<SubtypeRef>,
}

/// A reference from a supertype to one of its subtypes
#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
struct SubtypeRef {
    #[serde(rename = "type")]
    kind: String,
    named: bool,
}

/// Merges the `node-types.json` of several grammars of one language
///
/// A language can need more than one grammar. TypeScript is one. Its JSX
/// syntax conflicts with the angle-bracket type assertion, so `.tsx` files
/// parse with a second grammar, which drops the assertion and adds JSX. A
/// rule written for the language should run on both, so the lint pass
/// trait is generated from the union of their node kinds.
///
/// A kind that appears in several grammars appears once, and a supertype
/// lists every subtype any grammar gives it. Entries keep the order in
/// which they first appear. The output, and the fingerprint of the trait
/// generated from it, therefore depend only on the input.
///
/// The output keeps only the fields [`generate_visitor`] reads: the kind,
/// whether it is named, and its subtypes.
///
/// [`generate_visitor`]: crate::generate_visitor
///
/// # Errors
///
/// Returns an error if any input is not a valid `node-types.json`.
///
/// # Examples
///
/// ```
/// let one = r#"[{"type": "a", "named": true}]"#;
/// let two = r#"[{"type": "a", "named": true}, {"type": "b", "named": true}]"#;
///
/// let merged = whisker_codegen::merge_node_types(&[one, two]).unwrap();
///
/// assert_eq!(
///     merged,
///     r#"[{"type":"a","named":true},{"type":"b","named":true}]"#
/// );
/// ```
pub fn merge_node_types(grammars: &[&str]) -> Result<String, serde_json::Error> {
    let mut merged: Vec<NodeType> = Vec::new();

    for grammar in grammars {
        let node_types: Vec<NodeType> = serde_json::from_str(grammar)?;

        for node in node_types {
            let NodeType {
                kind,
                named,
                subtypes,
            } = node;

            let Some(existing) = merged
                .iter_mut()
                .find(|entry| entry.kind == kind && entry.named == named)
            else {
                merged.push(NodeType {
                    kind,
                    named,
                    subtypes,
                });
                continue;
            };

            for subtype in subtypes {
                if !existing.subtypes.contains(&subtype) {
                    existing.subtypes.push(subtype);
                }
            }
        }
    }

    serde_json::to_string(&merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPESCRIPT: &str = r#"[
        {"type": "expression", "named": true, "subtypes": [
            {"type": "identifier", "named": true},
            {"type": "type_assertion", "named": true}
        ]},
        {"type": "identifier", "named": true},
        {"type": "type_assertion", "named": true},
        {"type": "=>", "named": false}
    ]"#;

    const TSX: &str = r#"[
        {"type": "expression", "named": true, "subtypes": [
            {"type": "identifier", "named": true},
            {"type": "jsx_element", "named": true}
        ]},
        {"type": "identifier", "named": true},
        {"type": "jsx_element", "named": true},
        {"type": "=>", "named": false}
    ]"#;

    fn kinds(merged: &str) -> Vec<NodeType> {
        serde_json::from_str(merged).expect("the merge should be valid JSON")
    }

    #[test]
    fn merge_node_types_keeps_a_shared_kind_once() {
        let merged = merge_node_types(&[TYPESCRIPT, TSX]).expect("should merge");

        let identifiers = kinds(&merged)
            .into_iter()
            .filter(|node| node.kind == "identifier")
            .count();
        assert_eq!(identifiers, 1);
    }

    #[test]
    fn merge_node_types_keeps_first_appearance_order() {
        let merged = merge_node_types(&[TYPESCRIPT, TSX]).expect("should merge");

        let order: Vec<String> = kinds(&merged).into_iter().map(|node| node.kind).collect();
        assert_eq!(
            order,
            vec![
                "expression",
                "identifier",
                "type_assertion",
                "=>",
                "jsx_element"
            ]
        );
    }

    #[test]
    fn merge_node_types_keeps_unnamed_and_named_kinds_apart() {
        let unnamed = r#"[{"type": "this", "named": false}]"#;
        let named = r#"[{"type": "this", "named": true}]"#;

        let merged = merge_node_types(&[unnamed, named]).expect("should merge");

        assert_eq!(kinds(&merged).len(), 2);
    }

    #[test]
    fn merge_node_types_unions_the_subtypes_of_a_supertype() {
        let merged = merge_node_types(&[TYPESCRIPT, TSX]).expect("should merge");

        let expression = kinds(&merged)
            .into_iter()
            .find(|node| node.kind == "expression")
            .expect("expression should survive the merge");
        let subtypes: Vec<String> = expression
            .subtypes
            .into_iter()
            .map(|subtype| subtype.kind)
            .collect();
        assert_eq!(
            subtypes,
            vec!["identifier", "type_assertion", "jsx_element"]
        );
    }

    #[test]
    fn merge_node_types_with_invalid_json_returns_error() {
        let error = merge_node_types(&[TYPESCRIPT, "not json"]).expect_err("should fail");

        assert!(error.is_syntax());
    }

    #[test]
    fn merge_node_types_with_one_grammar_keeps_its_kinds() {
        let merged = merge_node_types(&[TYPESCRIPT]).expect("should merge");

        assert_eq!(kinds(&merged).len(), 4);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<NodeType>();
        assert_send::<SubtypeRef>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<NodeType>();
        assert_sync::<SubtypeRef>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<NodeType>();
        assert_unpin::<SubtypeRef>();
    }
}
