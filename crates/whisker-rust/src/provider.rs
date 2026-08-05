use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use ra_ap_hir::{Adt, Enum, Function, HasAttrs, HirDisplay, Module, Semantics, Type, attach_db};
use ra_ap_ide_db::base_db::SourceDatabase as _;
use ra_ap_ide_db::famous_defs::FamousDefs;
use ra_ap_ide_db::{ChangeWithProcMacros, RootDatabase};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace};
use ra_ap_syntax::{AstNode, Edition, NodeOrToken, SyntaxNode, TextRange, TextSize, ast};
use ra_ap_vfs::{AbsPathBuf, FileExcluded, FileId, Vfs, VfsPath};
use whisker_types::{
    Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationMap, DecorationProvider,
    ProviderName,
};

use crate::decorations::{AdtFlags, ErrorType, FnSignature, ResolvedType, ReturnMode, TypePath};

/// Decoration provider for Rust using rust-analyzer
///
/// Loads a Cargo workspace into a rust-analyzer database at construction
/// time, then attaches type information to tree-sitter nodes during the
/// decorate phase. The workspace load is expensive and happens once; the
/// per-file decoration pass is relatively cheap.
///
/// The provider stores the workspace root. Every gap except
/// [`CoverageGap::StaleSource`] names this root.
///
/// # Examples
///
/// ```ignore
/// let provider = RustDecorationProvider::load(Path::new("."))?;
/// let coverage = provider.decorate(&tree)?;
/// ```
pub struct RustDecorationProvider {
    db: Mutex<RootDatabase>,
    vfs: Vfs,
    root: Arc<Path>,
}

impl RustDecorationProvider {
    /// Identifies this provider in diagnostics
    const NAME: ProviderName = ProviderName("rust");

    /// Loads a Cargo workspace for semantic analysis
    ///
    /// rust-analyzer discovers the nearest manifest above
    /// `workspace_root`, and Cargo expands that manifest to its whole
    /// workspace. A gap that names a workspace names the discovered
    /// root, not `workspace_root`. Analysis enables the `test` cfg, so
    /// code under `#[cfg(test)]` resolves like any other code.
    ///
    /// This is an expensive operation that builds rust-analyzer's internal
    /// database from the project's Cargo.toml. Call this once at startup.
    ///
    /// # Errors
    ///
    /// Returns an error if the current directory cannot be read, if
    /// `workspace_root` is not valid UTF-8, or if discovery finds no
    /// Cargo project. The load also fails when workspace resolution, the
    /// build scripts, or the analysis database build fails.
    pub fn load(workspace_root: &Path) -> anyhow::Result<Self> {
        let cargo_config = CargoConfig {
            sysroot: Some(ra_ap_project_model::RustLibSource::Discover),
            set_test: true,
            ..CargoConfig::default()
        };
        let load_config = LoadCargoConfig {
            load_out_dirs_from_check: true,
            with_proc_macro_server: ProcMacroServerChoice::None,
            prefill_caches: false,
            num_worker_threads: 1,
            proc_macro_processes: 0,
        };

        let cwd = std::env::current_dir().context("read the current directory")?;
        let discover_from = cwd.join(workspace_root);
        anyhow::ensure!(
            discover_from.to_str().is_some(),
            "{} is not valid UTF-8, and rust-analyzer only addresses UTF-8 paths",
            discover_from.display()
        );
        let discover_from = AbsPathBuf::assert_utf8(discover_from);
        let manifest = ProjectManifest::discover_single(&discover_from)
            .with_context(|| format!("discover a Cargo project at {}", workspace_root.display()))?;

        let mut workspace = ProjectWorkspace::load(manifest, &cargo_config, &|_msg| {})
            .with_context(|| {
                format!(
                    "resolve the Cargo workspace discovered from {}",
                    workspace_root.display()
                )
            })?;

        if load_config.load_out_dirs_from_check {
            let build_scripts = workspace
                .run_build_scripts(&cargo_config, &|_msg| {})
                .context("run build scripts for the workspace")?;
            workspace.set_build_scripts(build_scripts);
        }

        let root: Arc<Path> = Arc::from(workspace.workspace_root().as_ref() as &Path);

        let (mut db, vfs, _proc_macro_client) =
            ra_ap_load_cargo::load_workspace(workspace, &cargo_config.extra_env, &load_config)
                .context("build the analysis database for the workspace")?;

        give_text_to_files_that_have_none(&mut db, &vfs, &root);

        Ok(Self {
            db: Mutex::new(db),
            vfs,
            root,
        })
    }

    /// Creates a provider that holds an empty workspace
    ///
    /// The provider declines every file with
    /// [`CoverageGap::OutsideRoot`]. Do not use it as a fallback when
    /// [`RustDecorationProvider::load`] fails: the pipeline would then
    /// refuse every file.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let provider = RustDecorationProvider::empty();
    /// let coverage = provider.decorate(&tree)?;
    /// ```
    pub fn empty() -> Self {
        Self {
            db: Mutex::new(RootDatabase::default()),
            vfs: Vfs::default(),
            root: Arc::from(Path::new(".")),
        }
    }

    /// Classifies a file rust-analyzer never interned
    ///
    /// A path under the loaded root reports [`CoverageGap::Unreachable`],
    /// because the toolchain could have known the file and did not. Any
    /// other path reports [`CoverageGap::OutsideRoot`].
    fn gap_for_unknown_file(&self, file_path: &Path) -> CoverageGap {
        if file_path.starts_with(&*self.root) {
            CoverageGap::Unreachable {
                root: Arc::clone(&self.root),
            }
        } else {
            CoverageGap::OutsideRoot {
                root: Arc::clone(&self.root),
            }
        }
    }
}

/// Records lossy UTF-8 text for interned files under `root` that have none
///
/// The load interns a file whose bytes are not UTF-8, but records no
/// text for it. rust-analyzer panics when it later reads a file that has
/// no recorded text. The read can happen while rust-analyzer analyzes a
/// sibling file, so the guard in `decorate` cannot prevent the panic.
/// This function stores the bytes as lossy UTF-8 text, so the database
/// always has text to read. The source-match guard in `decorate` still
/// declines the repaired file, because its lossy text differs from the
/// text whisker parsed.
///
/// The check covers only files under `root`, so it does not read the
/// sysroot or the dependencies on every run. It covers every extension,
/// because `include_str!` makes rust-analyzer read non-Rust files as
/// well.
fn give_text_to_files_that_have_none(db: &mut RootDatabase, vfs: &Vfs, root: &Path) {
    let mut lossy = Vec::new();

    for (file_id, vfs_path) in vfs.iter() {
        let Some(path) = vfs_path.as_path() else {
            continue;
        };
        let path: &Path = path.as_ref();
        if !path.starts_with(root) {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if std::str::from_utf8(&bytes).is_ok() {
            continue;
        }
        lossy.push((file_id, String::from_utf8_lossy(&bytes).into_owned()));
    }

    if lossy.is_empty() {
        return;
    }

    let mut change = ChangeWithProcMacros::default();
    for (file_id, text) in lossy {
        change.change_file(file_id, Some(text));
    }
    db.apply_change(change);
}

impl DecorationProvider for RustDecorationProvider {
    fn name(&self) -> ProviderName {
        Self::NAME
    }

    /// Attaches type decorations to the tree's nodes, or declines the file
    ///
    /// Four conditions must hold before a single decoration is produced,
    /// and their order matters. The file must be interned by the VFS, or
    /// rust-analyzer has never heard of it. It must not be excluded, or
    /// every question the database can be asked about it panics: the VFS
    /// keeps an excluded file's identity and neither its contents nor a
    /// source root, so the check has to come before the database is touched
    /// at all, not merely before its text is read. It must belong to a
    /// module, because `parse_guess_edition` papers over a missing module by
    /// guessing the current edition and then resolves nothing. And the text
    /// the pipeline parsed must equal the text the database holds, because
    /// the byte ranges carried by every target index into rust-analyzer's
    /// parse of that text.
    ///
    /// Only the first two conditions are guards. The module check protects
    /// nothing, because establishing the module reads whatever sibling
    /// modules the crate declares; a file whose text the load dropped would
    /// already have brought the process down by then, which is why
    /// [`give_text_to_files_that_have_none`] repairs the database at load
    /// time, so the read here needs no careful ordering.
    ///
    /// # Errors
    ///
    /// Returns an error if the file path cannot be made absolute or the
    /// database lock is poisoned. A file this provider knows nothing about
    /// is a [`Coverage::NotCovered`] verdict, not an error.
    fn decorate(&self, tree: &DecoratedTree) -> anyhow::Result<Coverage> {
        let file_path = tree.file();
        let file_path = std::path::absolute(file_path)
            .with_context(|| format!("resolve {} to an absolute path", file_path.display()))?;

        let vfs_path = VfsPath::new_real_path(file_path.to_string_lossy().to_string());

        let Some((file_id, excluded)) = self.vfs.file_id(&vfs_path) else {
            return Ok(Coverage::NotCovered(self.gap_for_unknown_file(&file_path)));
        };

        match excluded {
            FileExcluded::Yes => {
                return Ok(Coverage::NotCovered(CoverageGap::ExcludedByToolchain {
                    root: Arc::clone(&self.root),
                }));
            }
            FileExcluded::No => {}
        }

        let db = self
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("database lock poisoned: {e}"))?;

        let mut targets = Vec::new();
        collect_targets(&tree.root_node(), &mut targets);

        let coverage = attach_db(&*db, || {
            let sema = Semantics::new(&*db);

            let Some(module) = sema.file_to_module_def(file_id) else {
                return Coverage::NotCovered(CoverageGap::Unreachable {
                    root: Arc::clone(&self.root),
                });
            };

            let db_text: &str = db.file_text(file_id).text(&*db);
            if db_text != tree.source() {
                return Coverage::NotCovered(CoverageGap::StaleSource);
            }

            Coverage::Covered(resolve_targets(&sema, file_id, module, &targets))
        });

        Ok(coverage)
    }
}

/// Resolves every collected target against rust-analyzer's parse
///
/// The caller has already established that `file_id` belongs to `module`
/// and that its recorded text matches the text the targets' byte ranges
/// were taken from.
///
/// `module` identifies the crate. The crate supplies the edition for
/// display and the `core` definitions of `Result` and `Option`. These
/// resolve once per file, not once per target.
fn resolve_targets(
    sema: &Semantics<'_, RootDatabase>,
    file_id: FileId,
    module: Module,
    targets: &[Target],
) -> DecorationMap {
    let source_file = sema.parse_guess_edition(file_id);
    let syntax = source_file.syntax().clone();

    let krate = module.krate(sema.db);
    let famous = FamousDefs(sema, krate);
    let ctx = DecorateCtx {
        sema,
        syntax: &syntax,
        edition: krate.edition(sema.db),
        core_result: famous.core_result_Result(),
        core_option: famous.core_option_Option(),
    };

    let mut decorations = DecorationMap::new();
    for target in targets {
        match target {
            Target::Function { node_id, range } => {
                if let Some(sig) = resolve_function(&ctx, *range) {
                    decorations.insert(*node_id, sig);
                }
            }
            Target::MatchScrutinee {
                scrutinee_range,
                scrutinee_node_id,
            } => {
                if let Some((ty, flags)) = resolve_match_scrutinee(&ctx, *scrutinee_range) {
                    decorations.insert(*scrutinee_node_id, ty);
                    if let Some(flags) = flags {
                        decorations.insert(*scrutinee_node_id, flags);
                    }
                }
            }
            Target::IfElse {
                branch_range,
                else_node_id,
            } => {
                if let Some(ty) = resolve_expr_type(&ctx, *branch_range) {
                    decorations.insert(*else_node_id, ty);
                }
            }
            Target::TryExpr {
                operand_range,
                operand_node_id,
            } => {
                if let Some(ty) = resolve_expr_type(&ctx, *operand_range) {
                    decorations.insert(*operand_node_id, ty);
                }
            }
        }
    }
    decorations
}

/// A node the provider asks rust-analyzer to resolve
///
/// Each variant pairs the byte range rust-analyzer inspects with the
/// tree-sitter node ID that receives the decoration. These can differ: the
/// provider types an `else` branch from its block but decorates the
/// enclosing `else_clause`.
///
/// A full range, not a start offset, selects the node in rust-analyzer's
/// tree. A start offset lands on one token, and the smallest expression
/// around that token is often wrong. In `foo().bar()`, the start offset
/// names the path `foo`, not the call.
enum Target {
    Function {
        node_id: usize,
        range: TextRange,
    },
    MatchScrutinee {
        scrutinee_range: TextRange,
        scrutinee_node_id: usize,
    },
    IfElse {
        branch_range: TextRange,
        else_node_id: usize,
    },
    TryExpr {
        operand_range: TextRange,
        operand_node_id: usize,
    },
}

/// Converts a byte range into rust-analyzer's 32-bit one
///
/// Rust-analyzer addresses text with 32-bit offsets. An offset that does
/// not fit yields [`None`], because a truncated offset would name some
/// other expression.
///
/// The conversion is a separate function so that a unit test can reach the
/// 32-bit boundary without a four-gigabyte file.
fn text_range_from(range: Range<usize>) -> Option<TextRange> {
    let Range { start, end } = range;
    let start = u32::try_from(start).ok()?;
    let end = u32::try_from(end).ok()?;

    Some(TextRange::new(TextSize::from(start), TextSize::from(end)))
}

/// Converts a tree-sitter node's byte range into rust-analyzer's
fn text_range_of(node: &DecoratedNode<'_>) -> Option<TextRange> {
    text_range_from(node.raw().byte_range())
}

fn collect_targets(node: &DecoratedNode<'_>, targets: &mut Vec<Target>) {
    match node.kind() {
        "function_item" => {
            if let Some(range) = text_range_of(node) {
                targets.push(Target::Function {
                    node_id: node.id(),
                    range,
                });
            }
        }
        "match_expression" => {
            if let Some(scrutinee) = node.child_by_field_name("value")
                && let Some(scrutinee_range) = text_range_of(&scrutinee)
            {
                targets.push(Target::MatchScrutinee {
                    scrutinee_range,
                    scrutinee_node_id: scrutinee.id(),
                });
            }
        }
        "if_expression" => {
            if let Some(alt) = node.child_by_field_name("alternative") {
                let block = alt.named_child(0).unwrap_or(alt.clone());
                if let Some(branch_range) = text_range_of(&block) {
                    targets.push(Target::IfElse {
                        branch_range,
                        else_node_id: alt.id(),
                    });
                }
            }
        }
        "try_expression" => {
            if let Some(operand) = node.named_child(0)
                && let Some(operand_range) = text_range_of(&operand)
            {
                targets.push(Target::TryExpr {
                    operand_range,
                    operand_node_id: operand.id(),
                });
            }
        }
        _ => {}
    }

    for child in node.named_children() {
        collect_targets(&child, targets);
    }
}

/// Shared state for resolving one file's targets
///
/// The edition and the `core` enums are per-crate, so `resolve_targets`
/// fills them once for the whole file.
struct DecorateCtx<'a, 'db> {
    sema: &'a Semantics<'db, RootDatabase>,
    syntax: &'a SyntaxNode,
    edition: Edition,
    core_result: Option<Enum>,
    core_option: Option<Enum>,
}

impl DecorateCtx<'_, '_> {
    fn display_type(&self, ty: &Type<'_>, func: &Function) -> String {
        let krate = func.module(self.sema.db).krate(self.sema.db);
        let target = krate.to_display_target(self.sema.db);
        format!("{}", ty.display(self.sema.db, target))
    }
}

/// Returns the smallest node in rust-analyzer's tree that covers `range`
///
/// The two parsers agree on byte offsets but not on tree shape, so this
/// function locates a tree-sitter node by the span it occupies. From the
/// smallest covering node, the callers climb to the smallest [`ast::Expr`]
/// or [`ast::Fn`] that encloses the whole span.
///
/// The function refuses an empty range. Tree-sitter synthesizes zero-width
/// nodes during error recovery, and a zero-width span sits inside some
/// token, so the broken node would take that token's type.
///
/// It also refuses a range past the end of `syntax`.
/// [`SyntaxNode::covering_element`] panics on such a range, and one bad
/// target would abort the whole run.
///
/// The function takes the tree instead of the whole [`DecorateCtx`] so
/// that unit tests can call it without a loaded workspace.
fn covering_node(syntax: &SyntaxNode, range: TextRange) -> Option<SyntaxNode> {
    if range.is_empty() || !syntax.text_range().contains_range(range) {
        return None;
    }

    match syntax.covering_element(range) {
        NodeOrToken::Node(node) => Some(node),
        NodeOrToken::Token(token) => token.parent(),
    }
}

fn find_enclosing_fn(ctx: &DecorateCtx<'_, '_>, range: TextRange) -> Option<Function> {
    let fn_node = covering_node(ctx.syntax, range)?
        .ancestors()
        .find_map(ast::Fn::cast)?;
    ctx.sema.to_def(&fn_node)
}

fn find_expr_covering(ctx: &DecorateCtx<'_, '_>, range: TextRange) -> Option<ast::Expr> {
    covering_node(ctx.syntax, range)?
        .ancestors()
        .find_map(ast::Expr::cast)
}

/// Returns the return type as seen from inside the function's body
///
/// For an `async fn`, that is the future's output. In the body, `?` and
/// `return` convert into the output, not into the opaque `impl Future`.
///
/// The projection uses [`Function::async_ret_type`], which returns a type
/// only for an `async fn`. A `Future`-implementation test would also match
/// `fn f() -> impl Future<Output = anyhow::Result<()>>`, where the body's
/// `?` does not target that [`Result<T, E>`].
///
/// [`Result<T, E>`]: std::result::Result
fn effective_return_type<'db>(
    ctx: &DecorateCtx<'_, 'db>,
    func: Function,
) -> (Option<Type<'db>>, ReturnMode) {
    if func.is_async(ctx.sema.db) {
        match func.async_ret_type(ctx.sema.db) {
            Some(ty) => (Some(ty), ReturnMode::Awaited),
            None => (None, ReturnMode::Opaque),
        }
    } else {
        (Some(func.ret_type(ctx.sema.db)), ReturnMode::Direct)
    }
}

/// Returns whether `ty` is an instance of the enum `target` names
///
/// The comparison is by identity, not by rendered name: a user's own
/// `myres::Result<T>` renders as `Result<..>` but does not match.
fn is_instance_of(ty: &Type<'_>, target: Option<Enum>) -> bool {
    let Some(target) = target else {
        return false;
    };

    match ty.as_adt() {
        Some(Adt::Enum(actual)) => actual == target,
        Some(Adt::Struct(_)) => false,
        Some(Adt::Union(_)) => false,
        None => false,
    }
}

/// Returns the path of `ty`'s definition, if it has one
///
/// The path is the definition's, not a re-export's.
fn type_path(ctx: &DecorateCtx<'_, '_>, ty: &Type<'_>) -> Option<TypePath> {
    let db = ctx.sema.db;
    let adt = ty.as_adt()?;
    let module = adt.module(db);
    let krate = module.krate(db).display_name(db)?;
    let modules: Vec<String> = module
        .path_segments(db)
        .map(|segment| segment.display(db, ctx.edition).to_string())
        .collect();
    let name = adt.name(db).display(db, ctx.edition).to_string();

    Some(TypePath::new(&krate.to_string(), modules, &name))
}

/// Classifies the `E` of a [`Result<T, E>`]
///
/// A type parameter becomes [`ErrorType::Generic`]; a type with no
/// definition path at all becomes [`ErrorType::Unnamed`]. Neither gets an
/// invented name, because callers compare the result against real paths.
///
/// [`Result<T, E>`]: std::result::Result
fn classify_error(ctx: &DecorateCtx<'_, '_>, ty: &Type<'_>) -> ErrorType {
    match type_path(ctx, ty) {
        Some(path) => ErrorType::Named(path),
        None => match ty.as_type_param(ctx.sema.db) {
            Some(_) => ErrorType::Generic,
            None => ErrorType::Unnamed,
        },
    }
}

/// Returns the classified `E` when `ty` really is `core::result::Result<T, E>`
fn result_error_type(ctx: &DecorateCtx<'_, '_>, ty: &Type<'_>) -> Option<ErrorType> {
    let core_result = ctx.core_result?;
    let (adt, args) = ty.as_adt_with_args()?;
    let Adt::Enum(actual) = adt else {
        return None;
    };
    if actual != core_result {
        return None;
    }

    let error = args.into_iter().nth(1)??;
    Some(classify_error(ctx, &error))
}

fn resolve_function(ctx: &DecorateCtx<'_, '_>, range: TextRange) -> Option<FnSignature> {
    let func = find_enclosing_fn(ctx, range)?;

    let (ret_type, return_mode) = effective_return_type(ctx, func);
    let Some(ret_type) = ret_type else {
        return Some(FnSignature::new(None, None, return_mode));
    };

    let display = ctx.display_type(&ret_type, &func);
    let error_type = result_error_type(ctx, &ret_type);

    let resolved = ResolvedType::new(display)
        .with_result(is_instance_of(&ret_type, ctx.core_result))
        .with_never(ret_type.is_never());

    Some(FnSignature::new(Some(resolved), error_type, return_mode))
}

fn resolve_match_scrutinee(
    ctx: &DecorateCtx<'_, '_>,
    scrutinee_range: TextRange,
) -> Option<(ResolvedType, Option<AdtFlags>)> {
    let expr_node = find_expr_covering(ctx, scrutinee_range)?;
    let ty_info = ctx.sema.type_of_expr(&expr_node)?;

    let ty = ty_info.original;
    let is_enum = ty.as_adt().is_some_and(|adt| matches!(adt, Adt::Enum(_)));

    let func = find_enclosing_fn(ctx, scrutinee_range);
    let display = match &func {
        Some(f) => ctx.display_type(&ty, f),
        None => format!("{ty:?}"),
    };

    let resolved = ResolvedType::new(display).with_enum(is_enum);

    let flags = if let Some(Adt::Enum(e)) = ty.as_adt() {
        let krate = e.module(ctx.sema.db).krate(ctx.sema.db);
        let local_krate = func.map(|f| f.module(ctx.sema.db).krate(ctx.sema.db));
        let is_external = local_krate.is_some_and(|lk| lk != krate);
        let has_non_exhaustive = e.attrs(ctx.sema.db).is_non_exhaustive();
        Some(AdtFlags::new(has_non_exhaustive && is_external))
    } else {
        None
    };

    Some((resolved, flags))
}

fn resolve_expr_type(ctx: &DecorateCtx<'_, '_>, range: TextRange) -> Option<ResolvedType> {
    let expr = find_expr_covering(ctx, range)?;
    let ty_info = ctx.sema.type_of_expr(&expr)?;

    let ty = ty_info.original;
    let func = find_enclosing_fn(ctx, range);
    let display = match &func {
        Some(f) => ctx.display_type(&ty, f),
        None => format!("{ty:?}"),
    };

    let is_result = is_instance_of(&ty, ctx.core_result);
    let is_option = is_instance_of(&ty, ctx.core_option);
    let is_never = ty.is_never();

    Some(
        ResolvedType::new(display)
            .with_result(is_result)
            .with_option(is_option)
            .with_never(is_never),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ra_ap_syntax::SourceFile;

    use super::*;

    /// The source every `covering_node` test locates a range in
    ///
    /// `value` is several bytes long, so an empty range can sit strictly
    /// inside a token, where only the guard prevents a match.
    const COVERING_SOURCE: &str = "fn main() { let value = 1; }";

    fn covering_source_tree() -> SyntaxNode {
        SourceFile::parse(COVERING_SOURCE, Edition::CURRENT)
            .tree()
            .syntax()
            .clone()
    }

    fn offset_of(needle: &str) -> u32 {
        let found = COVERING_SOURCE
            .find(needle)
            .unwrap_or_else(|| panic!("the fixture source should contain {needle}"));

        u32::try_from(found).expect("the fixture source is small")
    }

    fn range_at(offset: u32, len: u32) -> TextRange {
        TextRange::new(TextSize::from(offset), TextSize::from(offset + len))
    }

    /// Parses `source` as the contents of `file_path`, without decorating it
    fn parse_rust(source: &str, file_path: PathBuf) -> DecoratedTree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&crate::language())
            .expect("the Rust grammar should load");
        let tree = parser.parse(source, None).expect("should parse");

        DecoratedTree::new(tree, source.to_string(), file_path)
    }

    /// Exercises the exclusion guard, which no fixture workspace can reach
    ///
    /// The VFS keeps an excluded file's identity but no contents, so any
    /// database query about it panics. The test seeds the state directly,
    /// because the load whisker performs never marks a file excluded.
    #[test]
    fn decorate_with_excluded_file_reports_excluded_by_toolchain() {
        let file_path =
            std::path::absolute("excluded_by_toolchain.rs").expect("should be made absolute");
        let mut vfs = Vfs::default();
        vfs.insert_excluded_file(VfsPath::new_real_path(
            file_path.to_string_lossy().to_string(),
        ));
        let provider = RustDecorationProvider {
            db: Mutex::new(RootDatabase::default()),
            vfs,
            root: Arc::from(Path::new("/")),
        };
        let tree = parse_rust("fn main() {}\n", file_path);

        let coverage = provider.decorate(&tree).expect("decorate should succeed");

        match coverage {
            Coverage::Covered(_) => panic!("an excluded file must not be covered"),
            Coverage::NotCovered(CoverageGap::ExcludedByToolchain { .. }) => {}
            Coverage::NotCovered(gap) => panic!("unexpected gap: {gap}"),
        }
    }

    /// A zero-width span must not take the type of the token around it
    ///
    /// Rowan resolves an empty range to the token that contains it. Without
    /// the guard, a zero-width node would take that token's type.
    #[test]
    fn covering_node_with_empty_range_returns_none() {
        let syntax = covering_source_tree();
        let inside_an_identifier = TextSize::from(offset_of("value") + 1);

        let covering = covering_node(
            &syntax,
            TextRange::new(inside_an_identifier, inside_an_identifier),
        );

        assert!(covering.is_none());
    }

    #[test]
    fn covering_node_with_range_inside_the_file_returns_a_node() {
        let syntax = covering_source_tree();
        let value = offset_of("value");

        let covering = covering_node(&syntax, range_at(value, 5)).expect("should cover `value`");

        assert!(covering.text_range().contains_range(range_at(value, 5)));
    }

    /// A span past the end of the tree must be skipped, not asserted on
    ///
    /// [`SyntaxNode::covering_element`] panics on a range outside the tree,
    /// so the guard skips the target instead of aborting the run.
    #[test]
    fn covering_node_with_range_past_the_end_returns_none() {
        let syntax = covering_source_tree();
        let past_the_end = u32::try_from(COVERING_SOURCE.len()).expect("fixture is small") + 1;

        let covering = covering_node(&syntax, range_at(past_the_end, 1));

        assert!(covering.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn load_with_non_utf8_path_returns_error() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;
        use std::path::PathBuf;

        let path = PathBuf::from(OsStr::from_bytes(b"whisker-\xff-not-utf8"));

        let Err(error) = RustDecorationProvider::load(&path) else {
            panic!("a path that is not UTF-8 should be rejected");
        };

        assert!(
            error.to_string().contains("not valid UTF-8"),
            "unexpected error: {error:#}"
        );
    }

    /// A span past the 32-bit limit must produce no target at all
    ///
    /// A truncated offset would name an unrelated span. The conversion is a
    /// separate function because a real file at this boundary would need
    /// four gigabytes.
    #[test]
    fn text_range_from_with_offset_past_u32_returns_none() {
        let past_u32 =
            usize::try_from(u64::from(u32::MAX) + 1).expect("this test assumes a 64-bit target");

        let converted = text_range_from(0..past_u32);

        assert!(converted.is_none());
    }

    #[test]
    fn text_range_from_with_offsets_within_u32_returns_the_range() {
        let converted = text_range_from(3..7).expect("a small range should convert");

        assert_eq!(converted, range_at(3, 4));
    }
}
