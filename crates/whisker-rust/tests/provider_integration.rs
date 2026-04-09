use std::path::{Path, PathBuf};

use whisker_rust::RustDecorationProvider;
use whisker_rust::decorations::{AdtFlags, FnSignature, ResolvedType};
use whisker_types::{DecoratedNode, DecoratedTree, DecorationProvider};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_project")
}

fn load_provider() -> RustDecorationProvider {
    RustDecorationProvider::load(&fixture_path()).expect("should load fixture project")
}

fn parse_and_decorate_fixture(provider: &RustDecorationProvider) -> DecoratedTree {
    let file_path = fixture_path().join("src/lib.rs");
    let source = std::fs::read_to_string(&file_path).expect("should read fixture source");

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&whisker_rust::language()).unwrap();
    let tree = parser.parse(&source, None).unwrap();

    let mut decorated = DecoratedTree::new(tree, source, file_path);
    provider
        .decorate(&mut decorated)
        .expect("decorate should succeed");
    decorated
}

fn find_function_by_name<'a>(root: &DecoratedNode<'a>, name: &str) -> Option<DecoratedNode<'a>> {
    for child in root.named_children() {
        if child.kind() == "function_item" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if name_node.text() == name {
                    return Some(child);
                }
            }
        }
        if let Some(found) = find_function_by_name(&child, name) {
            return Some(found);
        }
    }
    None
}

fn find_first_node_of_kind<'a>(node: &DecoratedNode<'a>, kind: &str) -> Option<DecoratedNode<'a>> {
    if node.kind() == kind {
        return Some(node.clone());
    }
    for child in node.named_children() {
        if let Some(found) = find_first_node_of_kind(&child, kind) {
            return Some(found);
        }
    }
    None
}

#[test]
fn provider_loads_fixture_project() {
    let _provider = load_provider();
}

#[test]
fn provider_decorates_without_panic() {
    let provider = load_provider();
    let _tree = parse_and_decorate_fixture(&provider);
}

#[test]
fn fn_signature_on_anyhow_result_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "returns_anyhow_result")
        .expect("should find returns_anyhow_result");

    let sig = func
        .decoration::<FnSignature>()
        .expect("should have FnSignature decoration");

    let ret = sig.return_type().expect("should have return type");
    assert!(
        ret.is_result(),
        "return type should be Result, got display: {}",
        ret.display()
    );
}

#[test]
fn fn_signature_on_io_result_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func =
        find_function_by_name(&root, "returns_io_result").expect("should find returns_io_result");

    let sig = func
        .decoration::<FnSignature>()
        .expect("should have FnSignature decoration");

    let ret = sig.return_type().expect("should have return type");
    assert!(
        ret.is_result(),
        "return type should be Result, got display: {}",
        ret.display()
    );
}

#[test]
fn fn_signature_on_non_result_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "bool_param").expect("should find bool_param");

    let sig = func
        .decoration::<FnSignature>()
        .expect("should have FnSignature decoration");

    let ret = sig.return_type().expect("should have return type");
    assert!(!ret.is_result(), "return type should not be Result");
    assert!(sig.error_type_name().is_none());
}

#[test]
fn match_scrutinee_on_enum_has_is_enum_true() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_enum").expect("should find match_on_enum");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");

    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");

    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("scrutinee should have ResolvedType");

    assert!(ty.is_enum(), "Color should be an enum");
}

#[test]
fn match_scrutinee_on_integer_has_is_enum_false() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func =
        find_function_by_name(&root, "match_on_integer").expect("should find match_on_integer");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");

    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");

    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("scrutinee should have ResolvedType");

    assert!(!ty.is_enum(), "i32 should not be an enum");
}

#[test]
fn local_enum_has_non_exhaustive_external_false() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_enum").expect("should find match_on_enum");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");

    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");

    let flags = scrutinee.decoration::<AdtFlags>();
    if let Some(flags) = flags {
        assert!(
            !flags.non_exhaustive_external(),
            "local enum should not be non_exhaustive_external"
        );
    }
}

#[test]
fn if_else_clause_gets_type_decoration() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "if_let_with_non_diverging_else")
        .expect("should find if_let_with_non_diverging_else");

    let if_expr =
        find_first_node_of_kind(&func, "if_expression").expect("should find if_expression");

    let else_clause = if_expr
        .child_by_field_name("alternative")
        .expect("should have else branch");

    let ty = else_clause
        .decoration::<ResolvedType>()
        .expect("else clause should have ResolvedType");

    assert!(
        !ty.is_never(),
        "non-diverging else should not be never type"
    );
}

#[test]
fn if_let_with_non_diverging_else_is_not_never() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "if_let_with_non_diverging_else")
        .expect("should find if_let_with_non_diverging_else");

    let if_expr =
        find_first_node_of_kind(&func, "if_expression").expect("should find if_expression");

    let else_clause = if_expr
        .child_by_field_name("alternative")
        .expect("should have else branch");

    let ty = else_clause.decoration::<ResolvedType>();
    if let Some(ty) = ty {
        assert!(
            !ty.is_never(),
            "non-diverging else should not have never type"
        );
    }
}

#[test]
fn every_function_item_has_fn_signature() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(&provider);
    let root = tree.root_node();

    let mut fn_count = 0;
    let mut sig_count = 0;

    for child in root.named_children() {
        if child.kind() == "function_item" {
            fn_count += 1;
            if child.decoration::<FnSignature>().is_some() {
                sig_count += 1;
            }
        }
    }

    assert!(fn_count > 0, "fixture should have functions");
    assert_eq!(
        fn_count, sig_count,
        "every function should have a FnSignature decoration"
    );
}
