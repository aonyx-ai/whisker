#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, LetStmt, MatchSource, StmtKind};
use rustc_lint::{LateContext, LateLintPass};

// r[impl lint.no-matches-macro.level]
dylint_linting::declare_late_lint! {
    /// ### What it does
    ///
    /// Flags uses of the `matches!` macro.
    ///
    /// ### Why is this bad?
    ///
    /// The `matches!` macro hides the full match expression, making it easy to
    /// miss unhandled variants when an enum gains new members. A full `match`
    /// expression forces you to consider each variant deliberately.
    ///
    /// ### Known problems
    ///
    /// None.
    ///
    /// ### Examples
    ///
    /// ```rust
    /// # enum Status { Active, Inactive, Pending }
    /// # let status = Status::Active;
    /// // Bad:
    /// let is_active = matches!(status, Status::Active);
    ///
    /// // Good:
    /// let is_active = match status {
    ///     Status::Active => true,
    ///     Status::Inactive | Status::Pending => false,
    /// };
    /// ```
    pub NO_MATCHES_MACRO,
    Warn,
    "uses of `matches!` macro hide unhandled variants"
}

// r[impl lint.no-matches-macro.detect]
/// Returns `true` if the given expression is a desugared `matches!` macro
/// invocation
fn is_matches_macro(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let ExprKind::Match(_, _, MatchSource::Normal) = expr.kind else {
        return false;
    };
    if !expr.span.from_expansion() {
        return false;
    }
    let expn_data = expr.span.ctxt().outer_expn_data();
    let Some(macro_def_id) = expn_data.macro_def_id else {
        return false;
    };
    cx.tcx.item_name(macro_def_id).as_str() == "matches"
}

// r[impl lint.no-matches-macro.message]
fn emit_matches_lint(cx: &LateContext<'_>, expr: &Expr<'_>) {
    span_lint_and_help(
        cx,
        NO_MATCHES_MACRO,
        expr.span.source_callsite(),
        "use of `matches!` macro",
        None,
        "use a full `match` expression instead",
    );
}

fn check_for_matches(cx: &LateContext<'_>, expr: &Expr<'_>) {
    if is_matches_macro(cx, expr) {
        emit_matches_lint(cx, expr);
    }
}

impl<'tcx> LateLintPass<'tcx> for NoMatchesMacro {
    fn check_local(&mut self, cx: &LateContext<'tcx>, local: &'tcx LetStmt<'_>) {
        let Some(init) = local.init else { return };
        check_for_matches(cx, init);
    }

    fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &'tcx rustc_hir::Stmt<'_>) {
        match stmt.kind {
            StmtKind::Expr(expr) | StmtKind::Semi(expr) => check_for_matches(cx, expr),
            StmtKind::Let(_) | StmtKind::Item(_) => {}
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        match expr.kind {
            ExprKind::Call(_, args) => {
                for arg in args {
                    check_for_matches(cx, arg);
                }
            }
            ExprKind::MethodCall(_, _, args, _) => {
                for arg in args {
                    check_for_matches(cx, arg);
                }
            }
            ExprKind::If(cond, _, _) => check_for_matches(cx, cond),
            ExprKind::Binary(_, lhs, rhs) => {
                check_for_matches(cx, lhs);
                check_for_matches(cx, rhs);
            }
            ExprKind::Unary(_, inner)
            | ExprKind::Field(inner, _)
            | ExprKind::AddrOf(_, _, inner)
            | ExprKind::Cast(inner, _)
            | ExprKind::Type(inner, _)
            | ExprKind::DropTemps(inner) => check_for_matches(cx, inner),
            ExprKind::Ret(Some(inner)) | ExprKind::Yield(inner, _) => {
                check_for_matches(cx, inner);
            }
            ExprKind::Array(exprs) | ExprKind::Tup(exprs) => {
                for e in exprs {
                    check_for_matches(cx, e);
                }
            }
            ExprKind::Assign(_, rhs, _) | ExprKind::AssignOp(_, _, rhs) => {
                check_for_matches(cx, rhs);
            }
            ExprKind::Block(block, _) => {
                if let Some(tail) = block.expr {
                    check_for_matches(cx, tail);
                }
            }
            ExprKind::Match(_, arms, _) => {
                for arm in arms {
                    check_for_matches(cx, arm.body);
                    if let Some(guard) = arm.guard {
                        check_for_matches(cx, guard);
                    }
                }
            }
            ExprKind::Index(base, idx, _) => {
                check_for_matches(cx, base);
                check_for_matches(cx, idx);
            }
            ExprKind::Struct(_, fields, _) => {
                for field in fields {
                    check_for_matches(cx, field.expr);
                }
            }
            ExprKind::Repeat(inner, _) => check_for_matches(cx, inner),
            ExprKind::ConstBlock(_)
            | ExprKind::Use(_, _)
            | ExprKind::Lit(_)
            | ExprKind::Let(_)
            | ExprKind::Loop(_, _, _, _)
            | ExprKind::Closure(_)
            | ExprKind::Path(_)
            | ExprKind::Break(_, _)
            | ExprKind::Continue(_)
            | ExprKind::Ret(None)
            | ExprKind::Become(_)
            | ExprKind::InlineAsm(_)
            | ExprKind::OffsetOf(_, _)
            | ExprKind::UnsafeBinderCast(_, _, _)
            | ExprKind::Err(_) => {}
        }
    }
}

#[test]
fn ui() {
    whisker_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
