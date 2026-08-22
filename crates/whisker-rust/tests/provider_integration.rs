#![cfg(feature = "provider")]

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use decoration_probes::{AnyhowBareTry, FunctionScopedImport, WildcardMatchArm};
use whisker_rust::decorations::{
    AdtFlags, FnSignature, ImportSource, ResolvedType, ReturnMode, TypePathRef,
};
use whisker_rust::{RustDecorationProvider, RustLintPassAdapter};
use whisker_types::{
    Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationProvider, Diagnostic, LintPass,
    RuleId, Severity, Span,
};

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

/// Writes a Cargo project whose `use` paths rustc rejects
///
/// Rustc refuses a struct as an import prefix, and it refuses an unknown
/// crate, so neither import can sit in a fixture that compiles. This
/// function generates them instead. Rust-analyzer names the struct and
/// resolves the unknown crate to nothing, and the provider must report
/// both outcomes rather than fall silent.
fn rejected_imports_project() -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rejected_imports_project");
    std::fs::create_dir_all(root.join("src")).expect("should create the fixture directories");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"rejected_imports_project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("should write the fixture manifest");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub struct Loader;\n\nimpl Loader {\n    pub fn load() -> u8 {\n        0\n    }\n}\n\npub fn import_an_associated_function() -> u8 {\n    use Loader::load;\n\n    load()\n}\n\npub fn import_from_an_unknown_crate() -> u8 {\n    use nowhere::thing;\n\n    thing()\n}\n",
    )
    .expect("should write the fixture crate root");

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

/// Returns the [`ImportSource`] on the first `use` inside the named function
fn import_source_in(tree: &DecoratedTree, function: &str) -> ImportSource {
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, function).unwrap_or_else(|| panic!("should find {function}"));
    let import = find_first_node_of_kind(&func, "use_declaration")
        .unwrap_or_else(|| panic!("{function} should hold a `use`"));

    *import
        .decoration::<ImportSource>()
        .unwrap_or_else(|| panic!("the `use` in {function} should have an ImportSource"))
}

/// Returns the operand of the first `?` expression inside `node`
fn find_first_try_operand<'a>(node: &DecoratedNode<'a>) -> Option<DecoratedNode<'a>> {
    find_first_node_of_kind(node, "try_expression")?.named_child(0)
}

/// Returns the source text covered by each diagnostic raised inside `func`
///
/// A diagnostic belongs to `func` when `func` contains the whole diagnostic
/// span. An expectation therefore does not depend on positions in the
/// diagnostics list. The helper returns source text, not offsets, so the
/// fixture can grow without renumbering every expectation.
fn flagged_within<'a>(
    tree: &'a DecoratedTree,
    diagnostics: &[Diagnostic],
    func: &DecoratedNode<'_>,
) -> Vec<&'a str> {
    let Range { start, end } = func.raw().byte_range();

    diagnostics
        .iter()
        .map(Diagnostic::span)
        .filter(|span| start <= span.start() && span.end() <= end)
        .map(|span| &tree.source()[span.start()..span.end()])
        .collect()
}

/// Returns the source text covered by every diagnostic in the file
///
/// A per-function expectation cannot notice a rule that fires somewhere
/// else. A whole-file expectation fails on any change in the rule's reach.
fn flagged_in_file<'a>(tree: &'a DecoratedTree, diagnostics: &[Diagnostic]) -> Vec<&'a str> {
    diagnostics
        .iter()
        .map(Diagnostic::span)
        .map(|span| &tree.source()[span.start()..span.end()])
        .collect()
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

/// A qualifier reports `Other` when it names another item, and
/// `Unresolved` when it names nothing
#[test]
fn decorate_with_imports_rustc_rejects_reports_other_and_unresolved() {
    let root = rejected_imports_project();
    let provider = RustDecorationProvider::load(&root).expect("should load the fixture");
    let file_path = root.join("src/lib.rs");
    let source = std::fs::read_to_string(&file_path).expect("should read the fixture crate root");
    let mut tree = parse_source(source, file_path);

    let coverage = provider.decorate(&tree).expect("decorate should succeed");

    match coverage {
        Coverage::Covered(decorations) => tree.merge_decorations(decorations),
        Coverage::NotCovered(gap) => panic!("the crate root should be covered, got: {gap}"),
    }
    assert_eq!(
        import_source_in(&tree, "import_an_associated_function"),
        ImportSource::Other
    );
    assert_eq!(
        import_source_in(&tree, "import_from_an_unknown_crate"),
        ImportSource::Unresolved
    );
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

/// An `async fn`'s signature must describe the awaited type
///
/// `Function::ret_type` reads the opaque future from the signature, and a
/// regression to it fails both the mode and the error-type assertions.
#[test]
fn fn_signature_on_async_function_reports_the_awaited_type() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "returns_anyhow_result_async")
        .expect("should find returns_anyhow_result_async");

    let sig = func
        .decoration::<FnSignature>()
        .expect("should have FnSignature decoration");

    assert_eq!(sig.return_mode(), ReturnMode::Awaited);
    let ret = sig.return_type().expect("should have return type");
    assert!(
        ret.is_result(),
        "the awaited type should be Result, got display: {}",
        ret.display()
    );
    let error = sig.error_type().expect("should have an error type");
    assert!(
        error.is(TypePathRef::new("anyhow", &[], "Error")),
        "the awaited error should be anyhow's, got: {error:?}"
    );
}

/// The error of `std::io::Result` resolves to `core::io::error::Error`
///
/// `std::io::Error` is a re-export, so the definition path names `core`.
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
    let error = sig.error_type().expect("should have an error type");
    assert!(
        error.is(TypePathRef::new("core", &["io", "error"], "Error")),
        "the error should be io's, got: {error:?}"
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
    assert!(sig.error_type().is_none());
}

#[test]
fn fn_signature_on_sync_function_reports_direct_mode() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "returns_anyhow_result")
        .expect("should find returns_anyhow_result");

    let sig = func
        .decoration::<FnSignature>()
        .expect("should have FnSignature decoration");

    assert_eq!(sig.return_mode(), ReturnMode::Direct);
}

/// A crate's own `Result` is not the standard one, however it renders
///
/// `myres::Result<()>` renders as `Result<()>`, which any prefix test on the
/// rendering accepts. Only a comparison against the enum `FamousDefs`
/// resolves can tell the two apart.
#[test]
fn fn_signature_on_user_defined_result_is_not_a_result() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "user_defined_result")
        .expect("should find user_defined_result");

    let sig = func
        .decoration::<FnSignature>()
        .expect("should have FnSignature decoration");

    let ret = sig.return_type().expect("should have return type");
    assert!(
        !ret.is_result(),
        "a user-defined Result is not core's, got display: {}",
        ret.display()
    );
    assert!(sig.error_type().is_none());
}

/// A list import and a glob import out of an enum both report the enum
///
/// Both shapes hold the qualifier as the use tree's own path, so one
/// lookup serves them.
#[test]
fn import_source_on_an_enum_qualifier_is_enum() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);

    assert_eq!(
        import_source_in(&tree, "import_enum_variants"),
        ImportSource::Enum
    );
    assert_eq!(
        import_source_in(&tree, "import_enum_variants_by_glob"),
        ImportSource::Enum
    );
}

#[test]
fn import_source_on_a_module_qualifier_is_module() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);

    assert_eq!(
        import_source_in(&tree, "import_from_module"),
        ImportSource::Module
    );
}

/// A module whose name starts with a capital is still a module
///
/// The name looks like a type, and only the resolution says otherwise.
#[test]
fn import_source_on_an_uppercase_module_is_module() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);

    assert_eq!(
        import_source_in(&tree, "import_from_uppercase_module"),
        ImportSource::Module
    );
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

/// A scrutinee reached through a field must be typed as the field
///
/// A lookup at the start offset of `palette.primary` finds the path
/// `palette` and reports the struct, not the enum.
#[test]
fn match_scrutinee_on_field_expression_has_is_enum_true() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_field_expression")
        .expect("should find match_on_field_expression");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");

    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");

    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("scrutinee should have ResolvedType");

    assert_eq!(scrutinee.text(), "palette.primary");
    assert!(
        ty.is_enum(),
        "the field's Color type should be an enum, got: {}",
        ty.display()
    );
}

/// A scrutinee returned by a call must be typed as the call's result
///
/// A lookup at the start offset of `pick_color()` finds the callee path,
/// whose type is the function itself, not the `Color` it returns.
#[test]
fn match_scrutinee_on_call_expression_has_is_enum_true() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "match_on_call_expression")
        .expect("should find match_on_call_expression");

    let match_expr =
        find_first_node_of_kind(&func, "match_expression").expect("should find match_expression");

    let scrutinee = match_expr
        .child_by_field_name("value")
        .expect("match should have value field");

    let ty = scrutinee
        .decoration::<ResolvedType>()
        .expect("scrutinee should have ResolvedType");

    assert_eq!(scrutinee.text(), "pick_color()");
    assert!(
        ty.is_enum(),
        "the returned Color type should be an enum, got: {}",
        ty.display()
    );
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

    let flags = scrutinee
        .decoration::<AdtFlags>()
        .expect("a local enum scrutinee should have AdtFlags");

    assert!(
        !flags.non_exhaustive_external(),
        "local enum should not be non_exhaustive_external"
    );
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

    let ty = else_clause
        .decoration::<ResolvedType>()
        .expect("else clause should have ResolvedType");

    assert!(
        !ty.is_never(),
        "non-diverging else should not have never type"
    );
}

/// The operand of `?` must be typed as the call, not as the callee
///
/// A lookup at the start offset of the operand finds the callee path,
/// whose type is a function, not a `Result`. Lints that require a `Result`
/// operand then never fire.
#[test]
fn try_operand_on_call_expression_is_typed_as_the_call_result() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func = find_function_by_name(&root, "returns_anyhow_result")
        .expect("should find returns_anyhow_result");

    let operand = find_first_try_operand(&func).expect("should find a `?` operand");

    let ty = operand
        .decoration::<ResolvedType>()
        .expect("`?` operand should have ResolvedType");

    assert_eq!(
        operand.text(),
        "std::fs::read_to_string(\"anyhow_bare.txt\")"
    );
    assert!(
        ty.is_result(),
        "read_to_string returns a Result, got: {}",
        ty.display()
    );
}

/// The operand of `?` must be typed as the method call, not as the receiver
///
/// A lookup at the start offset of `loader.load()?` finds the token
/// `loader`, whose type is a reference to the struct. This test pins the
/// method-call shape of the same fault.
#[test]
fn try_operand_on_method_call_is_typed_as_the_call_result() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let func =
        find_function_by_name(&root, "try_on_method_call").expect("should find try_on_method_call");

    let operand = find_first_try_operand(&func).expect("should find a `?` operand");

    let ty = operand
        .decoration::<ResolvedType>()
        .expect("`?` operand should have ResolvedType");

    assert_eq!(operand.text(), "loader.load()");
    assert!(
        ty.is_result(),
        "Loader::load returns a Result, got: {}",
        ty.display()
    );
}

/// A diagnostic that starts inside `func` but ends past it does not count
///
/// A lint reports one node, and nodes nest, so no fixture produces this
/// shape. The test builds the span by hand.
#[test]
fn flagged_within_with_a_span_past_the_function_end_returns_nothing() {
    let tree = parse_fixture_file("src/lib.rs");
    let root = tree.root_node();
    let func = find_function_by_name(&root, "match_on_enum").expect("should find match_on_enum");
    let Range { start, end } = func.raw().byte_range();
    let diagnostics = vec![Diagnostic::new(
        RuleId::new("lint.test"),
        Severity::Warn,
        "a span that leaves the function".to_string(),
        Span::new(tree.file().to_path_buf(), start, end + 1),
    )];

    let flagged = flagged_within(&tree, &diagnostics, &func);

    assert!(flagged.is_empty(), "unexpected matches: {flagged:?}");
}

/// The bare-try probe fires on decorations the provider made
///
/// The probe's own tests attach decorations by hand, so only an end-to-end
/// run shows that real decorations satisfy a rule that reads them. The
/// expectation lists everything the probe flags in the function, so the
/// `.context(..)` call next to the bare `?` must stay unflagged.
#[test]
fn anyhow_bare_try_flags_a_bare_try_in_an_anyhow_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_anyhow_result")
        .expect("should find returns_anyhow_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["std::fs::read_to_string(\"anyhow_bare.txt\")?"]
    );
}

/// An `async fn` that returns `anyhow::Result` is still an anyhow function
///
/// The signature output is the opaque future, so a provider that reads the
/// signature verbatim reports no error type and misses every `async fn`.
#[test]
fn anyhow_bare_try_flags_a_bare_try_in_an_async_anyhow_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_anyhow_result_async")
        .expect("should find returns_anyhow_result_async");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["std::fs::read_to_string(\"async_anyhow_bare.txt\")?"]
    );
}

/// An `async fn` in an impl block is reached the same way a free one is
///
/// The trait declares the method with an `impl Future` return type, a
/// different shape from the `async fn` that implements it. The awaited
/// type must come from the implementation, not the declaration.
#[test]
fn anyhow_bare_try_flags_a_bare_try_in_an_async_trait_method() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "load_async").expect("should find load_async");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["std::fs::read_to_string(&self.path)?"]
    );
}

#[test]
fn anyhow_bare_try_flags_a_bare_try_on_a_method_call() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, "try_on_method_call").expect("should find try_on_method_call");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["loader.load()?"]
    );
}

/// Only the anyhow function is flagged, out of four that all render `Error`
///
/// `syn::Result`, `std::fmt::Result`, `std::io::Result`, and
/// `anyhow::Result` all render their `E` as `Error`, so a rule that reads
/// the rendering flags all four. The `syn` case also covers an `Error`
/// whose definition path differs from its public re-export path.
#[test]
fn anyhow_bare_try_flags_only_the_anyhow_function_among_lookalike_errors() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    let anyhow_fn = find_function_by_name(&root, "returns_anyhow_result")
        .expect("should find returns_anyhow_result");
    let syn_fn =
        find_function_by_name(&root, "returns_syn_result").expect("should find returns_syn_result");
    let fmt_fn = find_function_by_name(&root, "fmt").expect("should find Display::fmt");
    let io_fn =
        find_function_by_name(&root, "returns_io_result").expect("should find returns_io_result");

    assert_eq!(
        flagged_within(&tree, &diagnostics, &anyhow_fn),
        vec!["std::fs::read_to_string(\"anyhow_bare.txt\")?"]
    );
    assert!(flagged_within(&tree, &diagnostics, &syn_fn).is_empty());
    assert!(flagged_within(&tree, &diagnostics, &fmt_fn).is_empty());
    assert!(flagged_within(&tree, &diagnostics, &io_fn).is_empty());
}

/// An `async fn` that returns `std::io::Result` is not an anyhow function
///
/// The async projection must identify the awaited error type, not just
/// make every `async fn` visible to the rule.
#[test]
fn anyhow_bare_try_ignores_a_bare_try_in_an_async_io_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_io_result_async")
        .expect("should find returns_io_result_async");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// A function that returns `std::io::Result` is outside this rule
///
/// `std::io::Result<()>` renders as `Result<(), Error>`, so a rule that
/// reads the rendering cannot tell this `Error` from anyhow's.
#[test]
fn anyhow_bare_try_ignores_a_bare_try_in_an_io_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, "returns_io_result").expect("should find returns_io_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// `Box<dyn Error>` occupies the `E` slot as an ADT, and it is not anyhow's
///
/// The `E` resolves to `Box`, defined in `alloc`, so the rule sees a named
/// error type with the wrong path.
#[test]
fn anyhow_bare_try_ignores_a_boxed_error() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_boxed_error")
        .expect("should find returns_boxed_error");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// An error type the signature leaves open cannot be anyhow's
///
/// A caller may instantiate `E` as `anyhow::Error`, but the body must
/// compile for every `E` the bounds admit. `.context(..)` is not available
/// there, so the rule reports nothing.
#[test]
fn anyhow_bare_try_ignores_a_generic_error() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_generic_error")
        .expect("should find returns_generic_error");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// A crate's own `Error` type must not match anyhow's
///
/// This shape produced most of the false positives when the rule first ran
/// over whisker's own source.
#[test]
fn anyhow_bare_try_ignores_a_local_error_type() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_local_error_result")
        .expect("should find returns_local_error_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// A `?` in a closure belongs to the closure, not to the function around it
///
/// The closure returns `std::io::Result`, so `.context(..)` on its `?`
/// would not compile. The function's own `read()?` is a genuine hit, so a
/// removed barrier fails the expectation with an extra entry.
#[test]
fn anyhow_bare_try_ignores_a_try_inside_a_closure() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "closure_returning_io_result")
        .expect("should find closure_returning_io_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(flagged_within(&tree, &diagnostics, &func), vec!["read()?"]);
}

/// Every place the rule fires in the fixture, listed once
///
/// Per-function expectations once stayed green while the rule fired on a
/// `std::io::Result` function nothing asserted about. The whole-file list
/// pins the rule's reach.
#[test]
fn anyhow_bare_try_over_the_whole_file_reports_only_anyhow_bodies() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowBareTry::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_in_file(&tree, &diagnostics),
        vec![
            "std::fs::read_to_string(\"anyhow_bare.txt\")?",
            "loader.load()?",
            "std::fs::read_to_string(\"async_anyhow_bare.txt\")?",
            "std::fs::read_to_string(&self.path)?",
            "read()?",
        ]
    );
}

/// Every place the function-scoped-import probe fires in the fixture
///
/// Among imports inside function bodies, the rule spares only those whose
/// qualifier resolves to an enum.
#[test]
fn function_scoped_import_over_the_whole_file_spares_only_variant_imports() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let mut passes: Vec<Box<dyn LintPass>> =
        vec![Box::new(RustLintPassAdapter::new(FunctionScopedImport))];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_in_file(&tree, &diagnostics),
        vec!["use std::collections::HashMap;", "use Shapes::draw;"]
    );
}

/// The wildcard probe fires on a scrutinee reached through a field
///
/// The old start-offset lookup only resolved scrutinees that are a bare
/// name. This test pins a shape that lookup used to miss.
#[test]
fn wildcard_match_arm_flags_a_scrutinee_reached_through_a_field() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "match_on_field_expression")
        .expect("should find match_on_field_expression");
    let mut passes: Vec<Box<dyn LintPass>> =
        vec![Box::new(RustLintPassAdapter::new(WildcardMatchArm))];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(flagged_within(&tree, &diagnostics, &func), vec!["_"]);
}

#[test]
fn wildcard_match_arm_flags_a_scrutinee_returned_by_a_call() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "match_on_call_expression")
        .expect("should find match_on_call_expression");
    let mut passes: Vec<Box<dyn LintPass>> =
        vec![Box::new(RustLintPassAdapter::new(WildcardMatchArm))];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(flagged_within(&tree, &diagnostics, &func), vec!["_"]);
}

/// The wildcard probe still ignores a non-enum scrutinee
///
/// A lookup that resolves too broadly would make `is_enum` true for
/// scrutinees that are not enums, and this test would catch that.
#[test]
fn wildcard_match_arm_ignores_a_scrutinee_that_is_not_an_enum() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, "match_on_integer").expect("should find match_on_integer");
    let mut passes: Vec<Box<dyn LintPass>> =
        vec![Box::new(RustLintPassAdapter::new(WildcardMatchArm))];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// Counts the fixture's `function_item` nodes and how many carry a signature
///
/// The walk recurses because a method inside an `impl` block is a
/// `function_item` too. A top-level loop would never count the `async`
/// trait method.
fn count_fn_signatures(node: &DecoratedNode<'_>, functions: &mut usize, signatures: &mut usize) {
    if node.kind() == "function_item" {
        *functions += 1;
        if node.decoration::<FnSignature>().is_some() {
            *signatures += 1;
        }
    }
    for child in node.named_children() {
        count_fn_signatures(&child, functions, signatures);
    }
}

#[test]
fn every_function_item_has_fn_signature() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();

    let mut fn_count = 0;
    let mut sig_count = 0;
    count_fn_signatures(&root, &mut fn_count, &mut sig_count);

    assert!(fn_count > 0, "fixture should have functions");
    assert_eq!(
        fn_count, sig_count,
        "every function should have a FnSignature decoration"
    );
}
