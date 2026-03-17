#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

// r[impl lint.prefer-let-else.level]
dylint_linting::declare_late_lint! {
    /// ### What it does
    ///
    /// Flags `if let` expressions where the `else` branch diverges (returns,
    /// breaks, continues, or panics), and suggests rewriting them using
    /// `let-else` syntax.
    ///
    /// ### Why is this bad?
    ///
    /// `let-else` makes the "happy path" obvious and reduces nesting. When the
    /// `else` branch always diverges, the intent is an early exit on pattern
    /// mismatch, which `let-else` expresses more directly.
    ///
    /// ### Known problems
    ///
    /// None.
    ///
    /// ### Examples
    ///
    /// ```rust
    /// # fn example(value: Option<i32>) -> Option<i32> {
    /// // Bad:
    /// let x = if let Some(v) = value {
    ///     v
    /// } else {
    ///     return None;
    /// };
    ///
    /// // Good:
    /// let Some(x) = value else {
    ///     return None;
    /// };
    /// # Some(x)
    /// # }
    /// ```
    pub PREFER_LET_ELSE,
    Warn,
    "`if let` with a diverging `else` branch can be written as `let-else`"
}

impl<'tcx> LateLintPass<'tcx> for PreferLetElse {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        // r[impl lint.prefer-let-else.detect]
        let ExprKind::If(cond, _then_block, Some(else_expr)) = expr.kind else {
            return;
        };

        // r[impl lint.prefer-let-else.if-let-only]
        let ExprKind::Let(..) = cond.kind else {
            return;
        };

        // r[impl lint.prefer-let-else.diverging-else]
        let else_ty = cx.typeck_results().expr_ty(else_expr);
        if !else_ty.is_never() {
            return;
        }

        // r[impl lint.prefer-let-else.message]
        span_lint_and_help(
            cx,
            PREFER_LET_ELSE,
            expr.span,
            "`if let` with a diverging `else` can be rewritten as `let-else`",
            None,
            "use `let ... else { <diverge> };` instead",
        );
    }
}

#[test]
fn ui() {
    whisker_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
