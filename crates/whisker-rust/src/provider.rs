use std::path::Path;
use std::sync::Mutex;

use anyhow::Context as _;
use ra_ap_hir::{Adt, HasAttrs, HirDisplay, Semantics, attach_db};
use ra_ap_ide_db::RootDatabase;
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice};
use ra_ap_project_model::CargoConfig;
use ra_ap_syntax::AstNode;
use ra_ap_syntax::ast;
use ra_ap_vfs::Vfs;
use whisker_types::{DecoratedNode, DecoratedTree, DecorationProvider};

use crate::decorations::{AdtFlags, FnSignature, ResolvedType};

/// Decoration provider for Rust using rust-analyzer
///
/// Loads a Cargo workspace into a rust-analyzer database at construction
/// time, then attaches type information to tree-sitter nodes during the
/// decorate phase. The workspace load is expensive and happens once; the
/// per-file decoration pass is relatively cheap.
///
/// # Examples
///
/// ```ignore
/// let provider = RustDecorationProvider::load(Path::new("."))?;
/// provider.decorate(&mut tree)?;
/// ```
pub struct RustDecorationProvider {
    db: Mutex<RootDatabase>,
    vfs: Vfs,
}

impl RustDecorationProvider {
    /// Loads a Cargo workspace for semantic analysis
    ///
    /// This is an expensive operation that builds rust-analyzer's internal
    /// database from the project's Cargo.toml. Call this once at startup.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace cannot be loaded (missing
    /// Cargo.toml, dependency resolution failure, etc.).
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

        let manifest = workspace_root.join("Cargo.toml");
        let manifest = if manifest.exists() {
            manifest
        } else {
            workspace_root.to_path_buf()
        };

        let (db, vfs, _proc_macro_client) =
            ra_ap_load_cargo::load_workspace_at(&manifest, &cargo_config, &load_config, &|_msg| {})
                .context("failed to load Cargo workspace for analysis")?;

        Ok(Self {
            db: Mutex::new(db),
            vfs,
        })
    }

    /// Creates a no-op provider that attaches no decorations
    ///
    /// Useful for testing syntax-only lints or when semantic analysis is
    /// not needed.
    pub fn empty() -> Self {
        Self {
            db: Mutex::new(RootDatabase::default()),
            vfs: Vfs::default(),
        }
    }
}

impl DecorationProvider for RustDecorationProvider {
    /// Attaches type decorations to the tree's nodes
    ///
    /// # Errors
    ///
    /// Returns an error if semantic analysis fails.
    fn decorate(&self, tree: &mut DecoratedTree) -> anyhow::Result<()> {
        let file_path = tree.file();
        let file_path = std::path::absolute(file_path).unwrap_or_else(|_| file_path.to_path_buf());

        let vfs_path = ra_ap_vfs::VfsPath::new_real_path(file_path.to_string_lossy().to_string());
        let Some((file_id, _excluded)) = self.vfs.file_id(&vfs_path) else {
            return Ok(());
        };

        let db = self
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("database lock poisoned: {e}"))?;

        let mut targets = Vec::new();
        collect_targets(&tree.root_node(), &mut targets);

        let results: Vec<Decoration> = attach_db(&*db, || {
            let sema = Semantics::new(&*db);
            let source_file = sema.parse_guess_edition(file_id);
            let syntax = source_file.syntax().clone();

            let ctx = DecorateCtx {
                sema: &sema,
                syntax: &syntax,
            };

            let mut decorations = Vec::new();
            for target in &targets {
                match target {
                    Target::Function {
                        node_id,
                        start_byte,
                    } => {
                        if let Some(sig) = resolve_function(&ctx, *start_byte) {
                            decorations.push(Decoration::FnSig(*node_id, sig));
                        }
                    }
                    Target::MatchScrutinee {
                        scrutinee_start_byte,
                        scrutinee_node_id,
                    } => {
                        if let Some((ty, flags)) =
                            resolve_match_scrutinee(&ctx, *scrutinee_start_byte)
                        {
                            decorations.push(Decoration::Type(*scrutinee_node_id, ty));
                            if let Some(f) = flags {
                                decorations.push(Decoration::Adt(*scrutinee_node_id, f));
                            }
                        }
                    }
                    Target::IfElse {
                        else_start_byte,
                        else_node_id,
                    } => {
                        if let Some(ty) = resolve_expr_type(&ctx, *else_start_byte) {
                            decorations.push(Decoration::Type(*else_node_id, ty));
                        }
                    }
                    Target::TryExpr {
                        operand_start_byte,
                        operand_node_id,
                    } => {
                        if let Some(ty) = resolve_expr_type(&ctx, *operand_start_byte) {
                            decorations.push(Decoration::Type(*operand_node_id, ty));
                        }
                    }
                }
            }
            decorations
        });

        for decoration in results {
            match decoration {
                Decoration::Type(id, ty) => tree.decorations_mut().insert(id, ty),
                Decoration::Adt(id, flags) => tree.decorations_mut().insert(id, flags),
                Decoration::FnSig(id, sig) => tree.decorations_mut().insert(id, sig),
            }
        }

        Ok(())
    }
}

enum Decoration {
    Type(usize, ResolvedType),
    Adt(usize, AdtFlags),
    FnSig(usize, FnSignature),
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

struct DecorateCtx<'a> {
    sema: &'a Semantics<'a, RootDatabase>,
    syntax: &'a ra_ap_syntax::SyntaxNode,
}

impl DecorateCtx<'_> {
    fn display_type(&self, ty: &ra_ap_hir::Type<'_>, func: &ra_ap_hir::Function) -> String {
        let krate = func.module(self.sema.db).krate(self.sema.db);
        let target = krate.to_display_target(self.sema.db);
        format!("{}", ty.display(self.sema.db, target))
    }
}

fn find_enclosing_fn(
    ctx: &DecorateCtx<'_>,
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

fn find_expr_at(ctx: &DecorateCtx<'_>, offset: ra_ap_syntax::TextSize) -> Option<ast::Expr> {
    ctx.syntax
        .token_at_offset(offset)
        .right_biased()
        .and_then(|t| t.parent_ancestors().find_map(ast::Expr::cast))
}

fn resolve_function(ctx: &DecorateCtx<'_>, start_byte: usize) -> Option<FnSignature> {
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
    ctx: &DecorateCtx<'_>,
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

fn resolve_expr_type(ctx: &DecorateCtx<'_>, start_byte: usize) -> Option<ResolvedType> {
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
}
