use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow_missing_context::AnyhowMissingContext;
use whisker_rust::decorations::{AdtFlags, FnSignature, ResolvedType, ReturnMode, TypePathRef};
use whisker_rust::{RustDecorationProvider, RustLintPassAdapter};
use whisker_types::{
    Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationProvider, Diagnostic, LintPass,
};
use wildcard_match_arm::WildcardMatchArm;

static PROVIDER: OnceLock<RustDecorationProvider> = OnceLock::new();

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_project")
}

/// Writes a Cargo project whose crate reaches two files that are not UTF-8
///
/// The project is generated rather than committed because a file that is
/// not valid UTF-8 survives neither review nor most editors intact, and
/// because it must be a package of its own: rust-analyzer would otherwise
/// parse the bad module while building the definition map of whichever
/// crate owns it, and take the whole test binary down with it.
///
/// The two bad files are reached by the two different routes that make
/// rust-analyzer read a file's text. `src/bad.rs` is a module, read while
/// the definition map of the crate around it is built. `src/notes.md` is an
/// [`include_str!`] argument, read while the body that includes it is
/// inferred — a file whisker would never decorate, and whose extension is
/// therefore no reason to leave it broken.
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

/// Returns the operand of the first `?` expression inside `node`
fn find_first_try_operand<'a>(node: &DecoratedNode<'a>) -> Option<DecoratedNode<'a>> {
    find_first_node_of_kind(node, "try_expression")?.named_child(0)
}

/// Returns the source text covered by each diagnostic raised inside `func`
///
/// Diagnostics are attributed to a function by byte containment rather than
/// by their position in the list, because these tests are about which of the
/// fixture's functions a lint reaches at all. An expectation written against
/// list positions would survive a lint that stopped seeing one function and
/// started seeing another.
///
/// Returning the covered text rather than raw offsets also lets the fixture
/// grow without every expectation being renumbered.
fn flagged_within<'a>(
    tree: &'a DecoratedTree,
    diagnostics: &[Diagnostic],
    func: &DecoratedNode<'_>,
) -> Vec<&'a str> {
    let range = func.raw().byte_range();

    diagnostics
        .iter()
        .map(Diagnostic::span)
        .filter(|span| range.contains(&span.start()))
        .map(|span| &tree.source()[span.start()..span.end()])
        .collect()
}

/// Returns the source text covered by every diagnostic in the file
///
/// A per-function expectation can only fail on a function it names, so it
/// cannot notice a rule that started firing somewhere else. Listing the whole
/// file is the expectation that has to be updated whenever the rule's reach
/// changes, which is what makes an unintended change in reach visible.
fn flagged_in_file<'a>(tree: &'a DecoratedTree, diagnostics: &[Diagnostic]) -> Vec<&'a str> {
    diagnostics
        .iter()
        .map(Diagnostic::span)
        .map(|span| &tree.source()[span.start()..span.end()])
        .collect()
}

/// One unreadable file must not take the rest of its crate down with it
///
/// `ra_ap_load_cargo` interns a file whose bytes are not UTF-8 without
/// recording its text, and rust-analyzer panics when it later reads that
/// text — while building the definition map of the crate around it, or
/// while inferring a body that includes it, so the file that dies is never
/// the one whisker was asked about. Before the load repaired such files,
/// this test aborted the whole binary with "Unable to fetch file text for
/// `vfs::FileId`" rather than failing. The fixture carries one of each
/// kind, so narrowing the repair back to Rust source turns this red too.
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

/// Code inside `#[cfg(test)] mod tests` has to resolve like any other code
///
/// Cargo leaves `test` out of a crate's cfg options by default, which
/// resolves every such block to nothing while whisker still walks it and
/// reports the file clean. Roughly half the lines of a Rust codebase live
/// in those blocks, so this is the difference between linting a project and
/// linting the part of it that is not tested.
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
        Coverage::Covered(_) => panic!("a file outside the workspace must not be covered"),
        Coverage::NotCovered(CoverageGap::OutsideWorkspace { .. }) => {}
        Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
    }
}

/// A generated file under the root must not be called "outside" the root
///
/// `whisker check .` runs the workspace's build scripts, which create
/// `target`, so the next run offers the provider files under the workspace
/// root that rust-analyzer never interned. Reporting those as
/// [`CoverageGap::OutsideWorkspace`] prints a path plainly inside the root
/// next to a message claiming it is outside, and a remedy the user cannot
/// act on.
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

/// An `async fn`'s signature must describe what its body produces
///
/// `Function::ret_type` reads the signature output verbatim, which for an
/// `async fn` is the opaque future rather than the type `?` in the body
/// converts into. Reverting to it leaves the return mode at `Direct` and the
/// error type at [`None`], so both halves of this expectation fail loudly.
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
/// `match palette.primary` starts on the token `palette`, and the smallest
/// expression around that token is the path `palette`, whose type is the
/// struct. Typing the scrutinee from where it starts rather than from what
/// it spans therefore reports a struct, and every lint that asks whether the
/// scrutinee is an enum quietly gets the wrong answer.
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
/// `match pick_color()` starts on the token `pick_color`, whose smallest
/// enclosing expression is the callee path. Typing that yields the function
/// itself, not the `Color` it returns.
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
/// `std::fs::read_to_string("anyhow_bare.txt")?` starts on the token `std`,
/// and the smallest expression around that token is the callee path. Typing
/// the operand from where it starts therefore reports a function type, which
/// is not a `Result`, and every lint gated on the operand being a `Result`
/// gives up before it looks at anything else.
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
/// `loader.load()?` starts on the token `loader`, whose type is a reference
/// to the struct. This is the same fault as the callee case, reached through
/// a different shape, so it is pinned separately.
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

/// The `anyhow_missing_context` rule fires on decorations the provider made
///
/// Its own unit tests attach the decorations by hand, so they went on passing
/// for as long as the provider produced decorations that never satisfied the
/// rule. Running the real provider and the real rule over real source is the
/// only arrangement that can tell the two apart.
///
/// The expectation is the complete list of what the rule flags inside the
/// function, so it also pins the negative: the `?` on the neighboring
/// `.context(..)` call must not be reported.
#[test]
fn anyhow_missing_context_flags_a_bare_try_in_an_anyhow_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_anyhow_result")
        .expect("should find returns_anyhow_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["std::fs::read_to_string(\"anyhow_bare.txt\")?"]
    );
}

/// An `async fn` returning `anyhow::Result` is still an anyhow function
///
/// Its signature output is the opaque future, so a provider reading the
/// signature verbatim reports no error type and the rule gives up. Every
/// command in whisker's own CLI is `async`, which is why this shape is
/// pinned against the real provider rather than a hand-built signature.
#[test]
fn anyhow_missing_context_flags_a_bare_try_in_an_async_anyhow_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_anyhow_result_async")
        .expect("should find returns_anyhow_result_async");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["std::fs::read_to_string(\"async_anyhow_bare.txt\")?"]
    );
}

/// An `async fn` in an impl block is reached the same way a free one is
///
/// The trait declares the method as returning `impl Future`, which is a
/// different shape from the `async fn` that implements it. The awaited type
/// has to come from the implementation, not the declaration.
#[test]
fn anyhow_missing_context_flags_a_bare_try_in_an_async_trait_method() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "load_async").expect("should find load_async");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["std::fs::read_to_string(&self.path)?"]
    );
}

#[test]
fn anyhow_missing_context_flags_a_bare_try_on_a_method_call() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, "try_on_method_call").expect("should find try_on_method_call");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(
        flagged_within(&tree, &diagnostics, &func),
        vec!["loader.load()?"]
    );
}

/// Only the anyhow function is flagged, out of four that all render `Error`
///
/// `syn::Result`, `std::fmt::Result`, `std::io::Result`, and `anyhow::Result`
/// all put a type named `Error` in the `E` slot, and rust-analyzer renders
/// every one of them as `Error`. A rule deciding from the rendering flags all
/// four. This runs the real provider and the real rule over real source,
/// which is the only arrangement that can tell a fixed rule from a rule whose
/// hand-built fixtures happen to agree with it.
///
/// The `syn` case is the sharp one: `syn` defines `Error` in a private
/// `error` module and re-exports it at the root, so its definition path and
/// its public path differ.
#[test]
fn anyhow_missing_context_flags_only_the_anyhow_function_among_lookalike_errors() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

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

/// An `async fn` returning `std::io::Result` is not an anyhow function
///
/// Reading the awaited type has to preserve the discrimination, not just
/// restore the reach: a fix that made every `async fn` visible without also
/// identifying its error type would trade one wrong answer for another.
#[test]
fn anyhow_missing_context_ignores_a_bare_try_in_an_async_io_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_io_result_async")
        .expect("should find returns_io_result_async");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// A `?` in a function returning `std::io::Result` is not this rule's business
///
/// `std::io::Result<()>` renders as `Result<(), Error>`, so a rule reading the
/// rendering sees a bare `Error` and cannot tell it from anyhow's. This pins
/// the identity question against the real provider, where the rendering is
/// produced rather than hand-written.
#[test]
fn anyhow_missing_context_ignores_a_bare_try_in_an_io_function() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func =
        find_function_by_name(&root, "returns_io_result").expect("should find returns_io_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// `Box<dyn Error>` occupies the `E` slot as an ADT, and it is not anyhow's
///
/// The `E` here is `Box`, whose definition lives in `alloc`, so the rule sees
/// a named error type that simply is not the one it cares about.
#[test]
fn anyhow_missing_context_ignores_a_boxed_error() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_boxed_error")
        .expect("should find returns_boxed_error");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// An error type the signature leaves open cannot be anyhow's
///
/// A caller may well instantiate `E` as `anyhow::Error`, but the body must
/// compile for every `E` the bounds admit, so `.context(..)` is not available
/// there and the rule must stay quiet.
#[test]
fn anyhow_missing_context_ignores_a_generic_error() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_generic_error")
        .expect("should find returns_generic_error");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// A crate's own `Error` must not answer to anyhow's
///
/// This is the shape that produced four of the seven false positives when
/// the rule was first run over whisker's own source: a crate declaring
/// `Error` at some path of its own, in a crate that need not depend on
/// anyhow at all.
#[test]
fn anyhow_missing_context_ignores_a_local_error_type() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "returns_local_error_result")
        .expect("should find returns_local_error_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert!(flagged_within(&tree, &diagnostics, &func).is_empty());
}

/// A `?` in a closure belongs to the closure, not to the function around it
///
/// The closure returns `std::io::Result`, so `.context(..)` on its `?` would
/// not compile; the function's own `read()?` is a genuine hit. The
/// expectation carries both, so removing the barrier fails it by gaining an
/// entry rather than by losing one.
#[test]
fn anyhow_missing_context_ignores_a_try_inside_a_closure() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let root = tree.root_node();
    let func = find_function_by_name(&root, "closure_returning_io_result")
        .expect("should find closure_returning_io_result");
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

    let diagnostics = whisker_core::walk(&tree, &mut passes);

    assert_eq!(flagged_within(&tree, &diagnostics, &func), vec!["read()?"]);
}

/// Every place the rule fires in the fixture, listed once
///
/// A per-function expectation can only fail on the function it names, so the
/// suite stayed green while the binary reported `lint.anyhow-missing-context`
/// on a `std::io::Result` function nothing asserted about. One expectation
/// over the whole file is what makes the rule's reach, rather than its
/// behavior on four chosen functions, the thing under test.
#[test]
fn anyhow_missing_context_over_the_whole_file_reports_only_anyhow_bodies() {
    let provider = load_provider();
    let tree = parse_and_decorate_fixture(provider);
    let mut passes: Vec<Box<dyn LintPass>> = vec![AnyhowMissingContext::into_lint_pass()];

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

/// The `wildcard_match_arm` rule sees scrutinees it reaches through a field
///
/// The rule was already firing on the fixture, but only on scrutinees that
/// are a bare name, which is the one shape the old start-offset lookup got
/// right. This pins the shapes it used to miss.
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

/// Reaching more scrutinees must not mean reaching every scrutinee
///
/// A lookup that resolved to something too broad would make `is_enum` true
/// far too often, and this is the cheapest place to notice that.
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
/// `function_item` like any other, and the shapes this suite most depends on
/// — the method call behind `?`, the `async` trait method — are exactly the
/// ones a top-level-only loop never counts.
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
