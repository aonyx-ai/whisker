#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, MatchSource};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Symbol;
use rustc_span::sym;

// r[impl lint.anyhow-missing-context.level]
dylint_linting::declare_late_lint! {
    /// ### What it does
    ///
    /// Flags uses of the `?` operator on [`Result`] types without a preceding
    /// `.context()` or `.with_context()` call, but only when the enclosing
    /// function returns `Result<T, anyhow::Error>`
    ///
    /// ### Why is this bad?
    ///
    /// Using `?` without `.context()` propagates errors without adding
    /// information about what operation failed. Rich error context makes
    /// debugging significantly easier by providing a chain of explanations
    /// for how the error occurred.
    ///
    /// ### Known problems
    ///
    /// None.
    ///
    /// ### Examples
    ///
    /// ```rust,ignore
    /// use anyhow::Context;
    ///
    /// // Bad:
    /// let file = std::fs::read_to_string("config.toml")?;
    ///
    /// // Good:
    /// let file = std::fs::read_to_string("config.toml")
    ///     .context("reading config file")?;
    /// ```
    ///
    /// [`Result`]: std::result::Result
    pub ANYHOW_MISSING_CONTEXT,
    Warn,
    "use of `?` on Result without `.context()` in a function returning anyhow::Error"
}

impl<'tcx> LateLintPass<'tcx> for AnyhowMissingContext {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        // r[impl lint.anyhow-missing-context.detect]
        let ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_)) = expr.kind else {
            return;
        };

        let original_expr = extract_try_operand(scrutinee);

        // r[impl lint.anyhow-missing-context.option-ignored]
        let expr_ty = cx.typeck_results().expr_ty(original_expr);
        if !is_result_type(cx, expr_ty) {
            return;
        }

        // r[impl lint.anyhow-missing-context.anyhow-only]
        if !returns_anyhow_error(cx) {
            return;
        }

        // r[impl lint.anyhow-missing-context.context-allowed]
        // r[impl lint.anyhow-missing-context.with-context-allowed]
        if is_context_call(original_expr) {
            return;
        }

        // r[impl lint.anyhow-missing-context.message]
        span_lint_and_help(
            cx,
            ANYHOW_MISSING_CONTEXT,
            expr.span,
            "use of `?` on Result without error context",
            None,
            "add `.context(\"description\")` before `?` to provide error context",
        );
    }
}

fn extract_try_operand<'tcx>(scrutinee: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
    let ExprKind::Call(_, args) = scrutinee.kind else {
        return scrutinee;
    };
    if args.is_empty() {
        return scrutinee;
    }
    &args[0]
}

fn is_context_call(expr: &Expr<'_>) -> bool {
    let ExprKind::MethodCall(path_segment, _, _, _) = expr.kind else {
        return false;
    };
    let name = path_segment.ident.name;
    name == Symbol::intern("context") || name == Symbol::intern("with_context")
}

fn is_result_type<'tcx>(cx: &LateContext<'tcx>, ty: ty::Ty<'tcx>) -> bool {
    let ty::Adt(adt_def, _) = ty.kind() else {
        return false;
    };
    cx.tcx.is_diagnostic_item(sym::Result, adt_def.did())
}

fn returns_anyhow_error(cx: &LateContext<'_>) -> bool {
    let owner_id = cx
        .tcx
        .hir_enclosing_body_owner(cx.last_node_with_lint_attrs);
    let owner_ty = cx.tcx.type_of(owner_id).skip_binder();
    let fn_sig = match owner_ty.kind() {
        ty::FnDef(..) => cx.tcx.fn_sig(owner_id).skip_binder().skip_binder(),
        ty::Closure(_, args) => args.as_closure().sig().skip_binder(),
        _ => return false,
    };
    let ret_ty = fn_sig.output();

    let ty::Adt(adt_def, substs) = ret_ty.kind() else {
        return false;
    };
    if !cx.tcx.is_diagnostic_item(sym::Result, adt_def.did()) {
        return false;
    }

    let Some(error_ty) = substs.types().nth(1) else {
        return false;
    };

    is_anyhow_error(cx, error_ty)
}

fn is_anyhow_error<'tcx>(cx: &LateContext<'tcx>, ty: ty::Ty<'tcx>) -> bool {
    let ty::Adt(adt_def, _) = ty.kind() else {
        return false;
    };
    cx.tcx.def_path_str(adt_def.did()) == "anyhow::Error"
}

#[test]
fn ui() {
    whisker_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn examples() {
    dylint_testing::ui_test_examples(env!("CARGO_PKG_NAME"));
}
