use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Context as _;
use ra_ap_hir::{Adt, HasAttrs, HirDisplay, Semantics, attach_db};
use ra_ap_ide_db::RootDatabase;
use ra_ap_ide_db::base_db::SourceDatabase as _;
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace};
use ra_ap_syntax::AstNode;
use ra_ap_syntax::ast;
use ra_ap_vfs::{AbsPathBuf, FileExcluded, FileId, Vfs, VfsPath};
use whisker_types::{
    Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationMap, DecorationProvider,
    ProviderName,
};

use crate::decorations::{AdtFlags, FnSignature, ResolvedType};

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

        let (db, vfs, _proc_macro_client) =
            ra_ap_load_cargo::load_workspace(workspace, &cargo_config.extra_env, &load_config)
                .context("build the analysis database for the workspace")?;

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
    /// reading its text panics — the toolchain keeps an excluded file's
    /// identity but not its contents. It must belong to a module, because
    /// `parse_guess_edition` papers over a missing module by guessing the
    /// current edition and then resolves nothing. And the text the
    /// pipeline parsed must equal the text the database holds, because the
    /// byte offsets carried by every target index into rust-analyzer's
    /// parse of that text.
    ///
    /// Reading the file text is safe only after the exclusion and module
    /// checks: rustc accepts UTF-8 source only, so any non-excluded file
    /// reachable from a crate root has recorded contents.
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

            if sema.file_to_module_def(file_id).is_none() {
                return Coverage::NotCovered(CoverageGap::Unreachable {
                    root: Arc::clone(&self.root),
                });
            }

            let db_text: &str = db.file_text(file_id).text(&*db);
            if db_text != tree.source() {
                return Coverage::NotCovered(CoverageGap::StaleSource);
            }

            Coverage::Covered(resolve_targets(&sema, file_id, &targets))
        });

        Ok(coverage)
    }
}

/// Resolves every collected target against rust-analyzer's parse
///
/// The caller has already established that `file_id` belongs to a module
/// and that its recorded text matches the text the targets' byte offsets
/// were taken from.
fn resolve_targets(
    sema: &Semantics<'_, RootDatabase>,
    file_id: FileId,
    targets: &[Target],
) -> DecorationMap {
    let source_file = sema.parse_guess_edition(file_id);
    let syntax = source_file.syntax().clone();

    let ctx = DecorateCtx {
        sema,
        syntax: &syntax,
    };

    let mut decorations = DecorationMap::new();
    for target in targets {
        match target {
            Target::Function {
                node_id,
                start_byte,
            } => {
                if let Some(sig) = resolve_function(&ctx, *start_byte) {
                    decorations.insert(*node_id, sig);
                }
            }
            Target::MatchScrutinee {
                scrutinee_start_byte,
                scrutinee_node_id,
            } => {
                if let Some((ty, flags)) = resolve_match_scrutinee(&ctx, *scrutinee_start_byte) {
                    decorations.insert(*scrutinee_node_id, ty);
                    if let Some(flags) = flags {
                        decorations.insert(*scrutinee_node_id, flags);
                    }
                }
            }
            Target::IfElse {
                else_start_byte,
                else_node_id,
            } => {
                if let Some(ty) = resolve_expr_type(&ctx, *else_start_byte) {
                    decorations.insert(*else_node_id, ty);
                }
            }
            Target::TryExpr {
                operand_start_byte,
                operand_node_id,
            } => {
                if let Some(ty) = resolve_expr_type(&ctx, *operand_start_byte) {
                    decorations.insert(*operand_node_id, ty);
                }
            }
        }
    }
    decorations
}

enum Target {
    Function {
        node_id: usize,
        start_byte: usize,
    },
    MatchScrutinee {
        scrutinee_start_byte: usize,
        scrutinee_node_id: usize,
    },
    IfElse {
        else_start_byte: usize,
        else_node_id: usize,
    },
    TryExpr {
        operand_start_byte: usize,
        operand_node_id: usize,
    },
}

fn collect_targets(node: &DecoratedNode<'_>, targets: &mut Vec<Target>) {
    match node.kind() {
        "function_item" => {
            targets.push(Target::Function {
                node_id: node.id(),
                start_byte: node.raw().start_byte(),
            });
        }
        "match_expression" => {
            if let Some(scrutinee) = node.child_by_field_name("value") {
                targets.push(Target::MatchScrutinee {
                    scrutinee_start_byte: scrutinee.raw().start_byte(),
                    scrutinee_node_id: scrutinee.id(),
                });
            }
        }
        "if_expression" => {
            if let Some(alt) = node.child_by_field_name("alternative") {
                let block = alt.named_child(0).unwrap_or(alt.clone());
                targets.push(Target::IfElse {
                    else_start_byte: block.raw().start_byte(),
                    else_node_id: alt.id(),
                });
            }
        }
        "try_expression" => {
            if let Some(operand) = node.named_child(0) {
                targets.push(Target::TryExpr {
                    operand_start_byte: operand.raw().start_byte(),
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

struct DecorateCtx<'a, 'db> {
    sema: &'a Semantics<'db, RootDatabase>,
    syntax: &'a ra_ap_syntax::SyntaxNode,
}

impl DecorateCtx<'_, '_> {
    fn display_type(&self, ty: &ra_ap_hir::Type<'_>, func: &ra_ap_hir::Function) -> String {
        let krate = func.module(self.sema.db).krate(self.sema.db);
        let target = krate.to_display_target(self.sema.db);
        format!("{}", ty.display(self.sema.db, target))
    }
}

fn find_enclosing_fn(
    ctx: &DecorateCtx<'_, '_>,
    offset: ra_ap_syntax::TextSize,
) -> Option<ra_ap_hir::Function> {
    let fn_node = ctx
        .syntax
        .token_at_offset(offset)
        .right_biased()?
        .parent_ancestors()
        .find_map(ast::Fn::cast)?;
    ctx.sema.to_def(&fn_node)
}

fn find_expr_at(ctx: &DecorateCtx<'_, '_>, offset: ra_ap_syntax::TextSize) -> Option<ast::Expr> {
    ctx.syntax
        .token_at_offset(offset)
        .right_biased()
        .and_then(|t| t.parent_ancestors().find_map(ast::Expr::cast))
}

fn resolve_function(ctx: &DecorateCtx<'_, '_>, start_byte: usize) -> Option<FnSignature> {
    let offset = ra_ap_syntax::TextSize::from(start_byte as u32);
    let func = find_enclosing_fn(ctx, offset)?;

    let ret_type = func.ret_type(ctx.sema.db);
    let display = ctx.display_type(&ret_type, &func);
    let is_result = display.starts_with("Result<") || display.contains("::Result<");
    let is_never = ret_type.is_never();

    let resolved = ResolvedType::new(display.clone())
        .with_result(is_result)
        .with_never(is_never);

    let error_type_name = if is_result {
        extract_result_error_type(&display)
    } else {
        None
    };

    Some(FnSignature::new(Some(resolved), error_type_name))
}

fn resolve_match_scrutinee(
    ctx: &DecorateCtx<'_, '_>,
    scrutinee_start_byte: usize,
) -> Option<(ResolvedType, Option<AdtFlags>)> {
    let offset = ra_ap_syntax::TextSize::from(scrutinee_start_byte as u32);
    let expr_node = find_expr_at(ctx, offset)?;
    let ty_info = ctx.sema.type_of_expr(&expr_node)?;

    let ty = ty_info.original;
    let is_enum = ty.as_adt().is_some_and(|adt| matches!(adt, Adt::Enum(_)));

    let func = find_enclosing_fn(ctx, offset);
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

fn resolve_expr_type(ctx: &DecorateCtx<'_, '_>, start_byte: usize) -> Option<ResolvedType> {
    let offset = ra_ap_syntax::TextSize::from(start_byte as u32);
    let expr = find_expr_at(ctx, offset)?;
    let ty_info = ctx.sema.type_of_expr(&expr)?;

    let ty = ty_info.original;
    let func = find_enclosing_fn(ctx, offset);
    let display = match &func {
        Some(f) => ctx.display_type(&ty, f),
        None => format!("{ty:?}"),
    };

    let is_result = display.starts_with("Result<") || display.contains("::Result<");
    let is_option = display.starts_with("Option<") || display.contains("::Option<");
    let is_never = ty.is_never();

    Some(
        ResolvedType::new(display)
            .with_result(is_result)
            .with_option(is_option)
            .with_never(is_never),
    )
}

fn extract_result_error_type(display: &str) -> Option<String> {
    let inner = display.strip_prefix("Result<")?.strip_suffix('>')?;

    let mut depth = 0;
    for (i, ch) in inner.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                let error_part = inner[i + 1..].trim();
                return Some(error_part.to_string());
            }
            _ => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_result_error_type_simple() {
        assert_eq!(
            extract_result_error_type("Result<(), anyhow::Error>"),
            Some("anyhow::Error".into())
        );
    }

    #[test]
    fn extract_result_error_type_nested_generics() {
        assert_eq!(
            extract_result_error_type("Result<Vec<String>, std::io::Error>"),
            Some("std::io::Error".into())
        );
    }

    #[test]
    fn extract_result_error_type_no_match() {
        assert_eq!(extract_result_error_type("Option<i32>"), None);
    }

    #[test]
    fn extract_result_error_type_unit_ok() {
        assert_eq!(
            extract_result_error_type("Result<(), Box<dyn Error>>"),
            Some("Box<dyn Error>".into())
        );
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
}
