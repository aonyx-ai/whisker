use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use whisker_rust::RustDecorationProvider;
use whisker_rust::decorations::{AdtFlags, FnSignature, ResolvedType};
use whisker_types::{Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationProvider};

static PROVIDER: OnceLock<RustDecorationProvider> = OnceLock::new();

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_project")
}

/// Writes a Cargo project whose crate reaches two files that are not UTF-8
///
/// This function generates the project at run time, because a committed
/// file that is not valid UTF-8 rarely survives review or an editor
/// intact. The project is a separate package, so the bad module cannot
/// break analysis of any other crate.
///
/// The two files cover both routes on which rust-analyzer reads text.
/// It reads the module `src/bad.rs` when it builds the definition map.
/// It reads the [`include_str!`] target `src/notes.md` when it infers
/// the body that includes it.
fn not_utf8_project() -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("not_utf8_project");
    std::fs::create_dir_all(root.join("src")).expect("should create the fixture directories");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"not_utf8_project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("should write the fixture manifest");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub mod bad;\n\npub enum Color {\n    Red,\n    Green,\n}\n\npub fn match_on_enum(color: Color) {\n    let _notes = include_str!(\"notes.md\");\n    match color {\n        Color::Red => {}\n        _ => {}\n    }\n}\n",
    )
    .expect("should write the fixture crate root");
    std::fs::write(
        root.join("src/bad.rs"),
        b"pub fn f() -> u8 { b\"\xff\xfe\"[0] }\n",
    )
    .expect("should write the fixture module that is not UTF-8");
    std::fs::write(root.join("src/notes.md"), b"notes \xff\xfe\n")
        .expect("should write the fixture include_str target that is not UTF-8");

    root
}

/// Loads the fixture workspace once and lends it to every test
///
/// Loading runs `cargo metadata` and `cargo check` against the fixture. Doing
/// that per test meant every test thread invoking Cargo against one directory
/// at the same time, and the losers of that contention got a workspace whose
/// sysroot had not finished loading. Tests that only touch locally declared
/// types survived it; the ones resolving `std` types failed. Loading once
/// leaves nothing to contend over.
fn load_provider() -> &'static RustDecorationProvider {
    PROVIDER
        .get_or_init(|| RustDecorationProvider::load(&fixture_path()).expect("should load fixture"))
}

fn parse_source(source: String, file_path: PathBuf) -> DecoratedTree {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&whisker_rust::language()).unwrap();
    let tree = parser.parse(&source, None).unwrap();

    DecoratedTree::new(tree, source, file_path)
}

fn parse_fixture_file(relative: &str) -> DecoratedTree {
    let file_path = fixture_path().join(relative);
    let source = std::fs::read_to_string(&file_path).expect("should read fixture source");

    parse_source(source, file_path)
}

fn parse_and_decorate_fixture(provider: &RustDecorationProvider) -> DecoratedTree {
    let mut decorated = parse_fixture_file("src/lib.rs");

    let coverage = provider
        .decorate(&decorated)
        .expect("decorate should succeed");

    match coverage {
        Coverage::Covered(decorations) => decorated.merge_decorations(decorations),
        Coverage::NotCovered(gap) => panic!("fixture should be covered, got: {gap}"),
    }
    decorated
}

fn find_function_by_name<'a>(root: &DecoratedNode<'a>, name: &str) -> Option<DecoratedNode<'a>> {
    for child in root.named_children() {
        if child.kind() == "function_item"
            && let Some(name_node) = child.child_by_field_name("name")
            && name_node.text() == name
        {
            return Some(child);
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

/// Checks that a module that is not UTF-8 leaves its siblings covered
///
/// Before the load repaired such files, rust-analyzer panicked over the
/// missing file text and aborted the whole test binary. The fixture
/// holds a bad module and a bad [`include_str!`] target, so a repair
/// narrowed to Rust source also fails this test.
#[test]
fn decorate_with_a_module_that_is_not_utf8_covers_its_siblings() {
    let root = not_utf8_project();
    let provider = RustDecorationProvider::load(&root).expect("should load the fixture");
    let file_path = root.join("src/lib.rs");
    let source = std::fs::read_to_string(&file_path).expect("should read the fixture crate root");
    let mut tree = parse_source(source, file_path);

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(decorations) => tree.merge_decorations(decorations),
        Coverage::NotCovered(gap) => panic!("the crate root should be covered, got: {gap}"),
    }
    let root_node = tree.root_node();
    let func =
        find_function_by_name(&root_node, "match_on_enum").expect("should find the function");
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

/// Checks that code under `#[cfg(test)]` resolves like any other code
///
/// Cargo leaves `test` out of the cfg options by default. Without it,
/// every test block resolves to nothing and whisker reports the file
/// clean.
#[test]
fn decorate_with_code_under_cfg_test_resolves_types() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_enum_under_cfg_test")
        .expect("should find match_on_enum_under_cfg_test");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");
    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");
    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("a scrutinee under cfg(test) should have ResolvedType");
    assert!(ty.is_enum(), "Color should be an enum");
}

#[test]
fn decorate_with_file_outside_workspace_reports_outside_workspace() {
    let provider = load_provider();
    let tree = parse_source(
        "fn main() {}\n".to_string(),
        PathBuf::from("/tmp/not_a_real_file.rs"),
    );

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(_) => panic!("a file outside the loaded root must not be covered"),
        Coverage::NotCovered(CoverageGap::OutsideRoot { .. }) => {}
        Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
    }
}

/// Checks that a generated file under the root reports
/// [`CoverageGap::Unreachable`]
///
/// Build scripts create files under `target` that rust-analyzer never
/// interned. The [`CoverageGap::OutsideRoot`] verdict would print a
/// path inside the root next to a message that claims it is outside.
#[test]
fn decorate_with_generated_file_under_root_reports_unreachable() {
    let provider = load_provider();
    let tree = parse_source(
        "fn main() {}\n".to_string(),
        fixture_path().join("target/debug/build/generated_out/out/generated.rs"),
    );

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(_) => panic!("a file no crate reaches must not be covered"),
        Coverage::NotCovered(CoverageGap::Unreachable { .. }) => {}
        Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
    }
}

#[test]
fn decorate_with_modified_source_reports_stale_source() {
    let provider = load_provider();
    let file_path = fixture_path().join("src/lib.rs");
    let source = std::fs::read_to_string(&file_path).expect("should read fixture source");
    let tree = parse_source(
        format!("{source}\npub fn added_after_load() {{}}\n"),
        file_path,
    );

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(_) => panic!("text the toolchain never saw must not be covered"),
        Coverage::NotCovered(CoverageGap::StaleSource) => {}
        Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
    }
}

#[test]
fn decorate_with_orphan_file_reports_unreachable() {
    let provider = load_provider();
    let tree = parse_fixture_file("orphan.rs");

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(_) => panic!("a file no crate reaches must not be covered"),
        Coverage::NotCovered(CoverageGap::Unreachable { .. }) => {}
        Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
    }
}

#[test]
fn provider_loads_fixture_project() {
    let _provider = load_provider();
}

#[test]
fn provider_decorates_without_panic() {
    let provider = load_provider();
    let _tree = parse_and_decorate_fixture(provider);
}

#[test]
fn fn_signature_on_anyhow_result_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
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
    let tree = parse_and_decorate_fixture(provider);
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
    let tree = parse_and_decorate_fixture(provider);
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
    let tree = parse_and_decorate_fixture(provider);
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
    let tree = parse_and_decorate_fixture(provider);
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
    let tree = parse_and_decorate_fixture(provider);
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
    let tree = parse_and_decorate_fixture(provider);
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
    let tree = parse_and_decorate_fixture(provider);
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
    let tree = parse_and_decorate_fixture(provider);
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
