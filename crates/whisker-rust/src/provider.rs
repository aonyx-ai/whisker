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
/// The loaded workspace is also the boundary of what this provider can
/// honestly say anything about, so the root is kept around: every
/// [`CoverageGap`] it reports names the workspace that declined.
///
/// # Examples
///
/// ```ignore
/// let provider = RustDecorationProvider::load(Path::new("."))?;
/// let coverage = provider.decorate(&tree)?;
/// ```
// r[impl sdk.provider.toolchain-connection]
pub struct RustDecorationProvider {
    db: Mutex<RootDatabase>,
    vfs: Vfs,
    root: Arc<Path>,
}

impl RustDecorationProvider {
    /// The name this provider is reported under in diagnostics
    const NAME: ProviderName = ProviderName("rust");

    /// Loads a Cargo workspace for semantic analysis
    ///
    /// `workspace_root` is a starting point, not necessarily the answer:
    /// rust-analyzer discovers the nearest manifest above it and Cargo then
    /// expands that to the whole workspace. This spells out the steps that
    /// [`ra_ap_load_cargo::load_workspace_at`] would otherwise perform in
    /// one call, because the workspace it settles on is what every
    /// [`CoverageGap`] this provider reports has to name. Telling a user
    /// their file is outside "the workspace at src/lib.rs" helps nobody.
    ///
    /// This is an expensive operation that builds rust-analyzer's internal
    /// database from the project's Cargo.toml. Call this once at startup.
    ///
    /// `set_test` is on, which puts `test` in the cfg options of every crate
    /// the workspace owns. Cargo's own default is off, so leaving it there
    /// would resolve a `#[cfg(test)] mod tests` block to nothing: whisker
    /// would still walk the file and the rules that need types would quietly
    /// find none, over the half of a typical Rust codebase that lives in
    /// those blocks. Editors running rust-analyzer turn it on for the same
    /// reason.
    ///
    /// # Errors
    ///
    /// Returns an error if the current directory cannot be read, if
    /// `workspace_root` is not valid UTF-8, if no Cargo project can be
    /// discovered from it, or if any stage of the load fails: resolving the
    /// workspace, running its build scripts, or building the analysis
    /// database. Each stage names itself, because their remedies differ.
    // r[impl sdk.provider.scope]
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

    /// Creates a provider holding an empty workspace
    ///
    /// Its VFS knows no files and its root is relative, so no absolute file
    /// path can ever lie under it: every file is declined with
    /// [`CoverageGap::OutsideWorkspace`]. That is the honest description of
    /// what an unloaded provider can do, and it is why this constructor
    /// cannot be used as a fallback when [`RustDecorationProvider::load`]
    /// fails: the pipeline would refuse every file rather than report them
    /// all clean.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let provider = RustDecorationProvider::empty();
    ///
    /// match provider.decorate(&tree)? {
    ///     Coverage::Covered(_) => unreachable!("an empty VFS covers nothing"),
    ///     Coverage::NotCovered(CoverageGap::OutsideWorkspace { root }) => {
    ///         eprintln!("no workspace loaded at {}", root.display());
    ///     }
    ///     Coverage::NotCovered(gap) => eprintln!("declined: {gap}"),
    /// }
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
    /// A path under the loaded root is one the toolchain could have known
    /// about and did not: a file no crate reaches, or one that appeared
    /// after the load, such as build-script output under `target`. Calling
    /// that "outside the workspace" while printing a path plainly inside it
    /// sends the user looking for a second workspace that does not exist,
    /// so only a path that really is elsewhere gets that verdict.
    fn gap_for_unknown_file(&self, file_path: &Path) -> CoverageGap {
        if file_path.starts_with(&*self.root) {
            CoverageGap::Unreachable {
                root: Arc::clone(&self.root),
            }
        } else {
            CoverageGap::OutsideWorkspace {
                root: Arc::clone(&self.root),
            }
        }
    }
}

/// Gives a text to every interned file under `root` that has none
///
/// [`ra_ap_load_cargo`] interns a file whose bytes are not UTF-8 but drops
/// its text on the floor, and rust-analyzer's file store panics on a file
/// whose text was never recorded. The panic cannot be guarded where whisker
/// asks its questions: rust-analyzer reads a module's text while building
/// the definition map of the crate around it, so asking about a file is
/// enough to kill the process over one of its *siblings*, and what the user
/// sees is a file id with no path. Transcribing the bytes lossily leaves
/// the database total, which is the only shape in which the rest of the
/// workspace can be analyzed at all. The file itself stays undecoratable:
/// its lossy text cannot equal the text whisker parsed, so the source-match
/// guard in `decorate` declines it.
///
/// Only files under `root` are read. Covering the rest would mean re-reading
/// the sysroot and every dependency on every run, and a file in either that
/// rust-analyzer would parse is UTF-8 already, or the dependency would not
/// have compiled.
///
/// Extension is not one of the tests. Rust source is the only thing whisker
/// decorates, but it is not the only thing whose text rust-analyzer reads:
/// `include_str!` reads whatever path it is given, and the load interns
/// `.toml` and `.md` beside the `.rs` files. Repairing only Rust source
/// would leave `include_str!("notes.md")` able to end the run, and the few
/// manifests a workspace holds cost nothing to check.
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

// r[impl sdk.provider.translation]
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
    /// time rather than leaving the read here to be ordered carefully.
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
/// The module is what fixes the crate every question in this file is asked
/// on behalf of: the edition its names print in, and the `core` whose
/// `Result` and `Option` a type is compared against. Every function in one
/// file belongs to one crate, so resolving them once per file rather than
/// once per target is exact rather than merely cheap.
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

/// A node whose type or signature the provider will ask rust-analyzer about
///
/// Each variant carries the byte range of the syntax the question is about
/// and the tree-sitter node ID the answer belongs on. The two are not always
/// the same node: an `else` branch is typed from its block but decorated on
/// the enclosing `else_clause`, which is the node lints reach for.
///
/// The range is a range and not a start offset because rust-analyzer's tree
/// is asked which of its nodes the range covers. An offset only identifies a
/// token, and the innermost expression around a token is routinely not the
/// expression the range names: the offset of `foo().bar()` lands on `foo`,
/// whose innermost enclosing expression is the path `foo`, typed as a
/// function rather than as the call's result.
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
/// Rust-analyzer addresses text with 32-bit offsets, so a file too large for
/// one is a file it cannot describe. Returning [`None`] is then the only
/// honest answer, because a truncated offset would quietly name some other
/// expression and decorate this node with that expression's type.
///
/// The arithmetic is separate from the node access so that the refusal can be
/// tested. Fused to a [`DecoratedNode`], the only way to reach the boundary
/// would be a four-gigabyte fixture.
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

/// What every question about one file is asked against
///
/// The three resolved fields are per-crate, not per-target, and are held
/// rather than recomputed because they are the whole difference between
/// asking rust-analyzer what a type *is* and reading what it *printed*.
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
/// The two parsers agree on byte offsets — the caller has already checked
/// that they were handed the same text — but not on tree shape, so a
/// tree-sitter node is located by the span it occupies rather than by
/// position in the tree. Descending to the smallest covering node and then
/// climbing is what makes the lookup shape-independent: whichever node
/// rust-analyzer happens to have at that span, the ancestor walk from it
/// reaches the smallest [`ast::Expr`] or [`ast::Fn`] enclosing the whole
/// span, which is the one the caller meant.
///
/// An empty range is refused. Tree-sitter synthesizes zero-width nodes while
/// recovering from a parse error, and a zero-width span "covers" whatever
/// token it sits inside, which would decorate the broken node with an
/// unrelated neighbor's type.
///
/// A range reaching past the end of `syntax` is refused too, and it has to be
/// refused here: [`SyntaxNode::covering_element`] asserts rather than returns
/// on one, so the caller would abort instead of skipping a target.
///
/// The tree is taken rather than the whole [`DecorateCtx`] because both
/// refusals are properties of a range against a parse, and a parameter that
/// needs a loaded workspace to construct is one no unit test can reach.
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

/// Returns the type `?` and `return` answer to inside the function's body
///
/// For an `async fn` that is the future's output, not the future: an
/// `async fn`'s signature output is the opaque `impl Future`, which is not
/// what any expression in its body converts into.
///
/// The projection is deliberately reached through [`Function::async_ret_type`]
/// rather than through a "does this implement `Future`" test. The latter also
/// sees through `fn f() -> impl Future<Output = anyhow::Result<()>>`, where a
/// `?` in the body cannot target that [`Result<T, E>`] at all, and would
/// answer confidently and wrongly. Gating on `is_async`, which
/// `async_ret_type` does itself, is exactly the distinction that matters.
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
/// Identity, not rendering: a user's own `myres::Result<T>` renders as
/// `Result<..>` and is not this.
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

/// Returns where `ty`'s defining item lives, if it has one
///
/// The path is the definition's, not any re-export's, because a definition
/// only moves in a breaking release while a re-export can appear or vanish
/// in a patch one.
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
/// A type with no defining item is reported as such rather than given an
/// invented name, because the only use of this value is a comparison against
/// a path, and a name nothing declares can only make that comparison lie.
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
    /// `value` is more than one byte long, so an offset strictly inside it is
    /// available. That is what makes the empty-range refusal observable: at
    /// such an offset rowan does have an element to answer with, and the
    /// guard is the only reason nothing is returned.
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

    /// An excluded file keeps its identity and loses everything else
    ///
    /// The VFS records such a file as present but holds neither its contents
    /// nor a source root for it, so every question the database can be asked
    /// about it panics. Whisker cannot decline to have an opinion: the
    /// exclusion is reported by the same [`Vfs::file_id`] call that hands
    /// back the id, so the guard is the only thing between that state and a
    /// panic raised from inside rust-analyzer.
    ///
    /// The state is seeded directly because the load whisker performs never
    /// produces it — rust-analyzer's language server marks files excluded
    /// from its own configuration, while Cargo's `[workspace] exclude` keeps
    /// a path out of the VFS entirely rather than marking it. That makes
    /// this the one guard no fixture workspace can exercise, which is
    /// exactly why it needs a test of its own rather than trust.
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

    /// A zero-width span must not borrow the identity of the token around it
    ///
    /// Rowan answers `covering_element` for an empty range with the token it
    /// falls inside, so without the refusal the caller would decorate a
    /// tree-sitter node of no width with that token's type. The offset here
    /// sits in the middle of `value`, which is what makes the difference
    /// observable: the same offset with any width at all does resolve.
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

    /// A span the parse does not reach must be declined, not asserted on
    ///
    /// [`SyntaxNode::covering_element`] panics on a range outside the tree,
    /// so this guard is the difference between skipping one target and
    /// aborting the whole run.
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

    /// A span the 32-bit addressing cannot hold must produce no target at all
    ///
    /// Truncating instead would name a different, entirely unrelated span,
    /// and the decoration attached there would be confidently wrong rather
    /// than absent. Reaching the boundary through a real file would take a
    /// four-gigabyte fixture, which is why the arithmetic has its own seam.
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
