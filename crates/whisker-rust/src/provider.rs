use std::path::Path;
use std::sync::Mutex;

use anyhow::Context as _;
use ra_ap_hir::{Adt, HasAttrs, HirDisplay, Semantics};
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
// r[impl sdk.provider.toolchain-connection]
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
    // r[impl sdk.provider.scope]
    pub fn load(workspace_root: &Path) -> anyhow::Result<Self> {
        let cargo_config = CargoConfig::default();
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

// r[impl sdk.provider.translation]
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

        let sema = Semantics::new(&*db);
        let source_file = sema.parse_guess_edition(file_id);
        let syntax = source_file.syntax().clone();

        let ctx = DecorateCtx {
            sema: &sema,
            syntax: &syntax,
        };

        let mut targets = Vec::new();
        collect_targets(&tree.root_node(), &mut targets);

        for target in &targets {
            match target {
                Target::Function {
                    node_id,
                    start_byte,
                } => {
                    decorate_function(tree, &ctx, *node_id, *start_byte);
                }
                Target::MatchScrutinee {
                    start_byte,
                    scrutinee_start_byte,
                    scrutinee_node_id,
                } => {
                    decorate_match_scrutinee(
                        tree,
                        &ctx,
                        *start_byte,
                        *scrutinee_start_byte,
                        *scrutinee_node_id,
                    );
                }
                Target::IfElse {
                    else_start_byte,
                    else_node_id,
                } => {
                    decorate_if_else(tree, &ctx, *else_start_byte, *else_node_id);
                }
                Target::TryExpr {
                    operand_start_byte,
                    operand_node_id,
                } => {
                    decorate_try_expr(tree, &ctx, *operand_start_byte, *operand_node_id);
                }
            }
        }

        Ok(())
    }
}

enum Target {
    Function {
        node_id: usize,
        start_byte: usize,
    },
    MatchScrutinee {
        start_byte: usize,
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
                    start_byte: node.raw().start_byte(),
                    scrutinee_start_byte: scrutinee.raw().start_byte(),
                    scrutinee_node_id: scrutinee.id(),
                });
            }
        }
        "if_expression" => {
            if let Some(alt) = node.child_by_field_name("alternative") {
                targets.push(Target::IfElse {
                    else_start_byte: alt.raw().start_byte(),
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

fn decorate_function(
    tree: &mut DecoratedTree,
    ctx: &DecorateCtx<'_>,
    node_id: usize,
    start_byte: usize,
) {
    let offset = ra_ap_syntax::TextSize::from(start_byte as u32);

    let Some(func) = find_enclosing_fn(ctx, offset) else {
        return;
    };

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

    let sig = FnSignature::new(Some(resolved), error_type_name);
    tree.decorations_mut().insert(node_id, sig);
}

fn decorate_match_scrutinee(
    tree: &mut DecoratedTree,
    ctx: &DecorateCtx<'_>,
    _match_start_byte: usize,
    scrutinee_start_byte: usize,
    scrutinee_id: usize,
) {
    let offset = ra_ap_syntax::TextSize::from(scrutinee_start_byte as u32);

    let Some(expr_node) = find_expr_at(ctx, offset) else {
        return;
    };

    let Some(ty_info) = ctx.sema.type_of_expr(&expr_node) else {
        return;
    };

    let ty = ty_info.original;
    let is_enum = ty.as_adt().is_some_and(|adt| matches!(adt, Adt::Enum(_)));

    let func = find_enclosing_fn(ctx, offset);
    let display = match &func {
        Some(f) => ctx.display_type(&ty, f),
        None => format!("{ty:?}"),
    };

    let resolved = ResolvedType::new(display).with_enum(is_enum);
    tree.decorations_mut().insert(scrutinee_id, resolved);

    if let Some(Adt::Enum(e)) = ty.as_adt() {
        let krate = e.module(ctx.sema.db).krate(ctx.sema.db);
        let local_krate = func.map(|f| f.module(ctx.sema.db).krate(ctx.sema.db));
        let is_external = local_krate.is_some_and(|lk| lk != krate);

        let has_non_exhaustive = e.attrs(ctx.sema.db).is_non_exhaustive();

        let flags = AdtFlags::new(has_non_exhaustive && is_external);
        tree.decorations_mut().insert(scrutinee_id, flags);
    }
}

fn decorate_if_else(
    tree: &mut DecoratedTree,
    ctx: &DecorateCtx<'_>,
    else_start_byte: usize,
    else_node_id: usize,
) {
    let offset = ra_ap_syntax::TextSize::from(else_start_byte as u32);

    let Some(expr) = find_expr_at(ctx, offset) else {
        return;
    };

    let Some(ty_info) = ctx.sema.type_of_expr(&expr) else {
        return;
    };

    let ty = ty_info.original;
    let resolved = ResolvedType::new(String::new()).with_never(ty.is_never());

    tree.decorations_mut().insert(else_node_id, resolved);
}

fn decorate_try_expr(
    tree: &mut DecoratedTree,
    ctx: &DecorateCtx<'_>,
    operand_start_byte: usize,
    operand_node_id: usize,
) {
    let offset = ra_ap_syntax::TextSize::from(operand_start_byte as u32);

    let Some(expr) = find_expr_at(ctx, offset) else {
        return;
    };

    let Some(ty_info) = ctx.sema.type_of_expr(&expr) else {
        return;
    };

    let ty = ty_info.original;

    let func = find_enclosing_fn(ctx, offset);
    let display = match &func {
        Some(f) => ctx.display_type(&ty, f),
        None => format!("{ty:?}"),
    };

    let is_result = display.starts_with("Result<") || display.contains("::Result<");
    let is_option = display.starts_with("Option<") || display.contains("::Option<");

    let resolved = ResolvedType::new(display)
        .with_result(is_result)
        .with_option(is_option);

    tree.decorations_mut().insert(operand_node_id, resolved);
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
