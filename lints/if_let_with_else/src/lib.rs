#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

// r[impl lint.if-let-with-else.level]
dylint_linting::declare_late_lint! {
    /// ### What it does
    ///
    /// Flags `if let` expressions that have a non-diverging `else` branch,
    /// suggesting a `match` expression instead.
    ///
    /// ### Why is this bad?
    ///
    /// An `if let` with an `else` branch is equivalent to a two-arm `match`
    /// but obscures the pattern-matching intent. A `match` expression makes
    /// both arms equally visible and encourages exhaustive handling.
    ///
    /// `if let` without an `else` is acceptable for short, single-branch
    /// pattern actions.
    ///
    /// ### Known problems
    ///
    /// None.
    ///
    /// ### Examples
    ///
    /// ```rust
    /// # let value: Option<i32> = Some(42);
    /// // Bad:
    /// if let Some(x) = value {
    ///     println!("{x}");
    /// } else {
    ///     println!("none");
    /// }
    ///
    /// // Good:
    /// match value {
    ///     Some(x) => println!("{x}"),
    ///     None => println!("none"),
    /// }
    /// ```
    pub IF_LET_WITH_ELSE,
    Warn,
    "`if let` with `else` should be a `match` expression"
}

impl<'tcx> LateLintPass<'tcx> for IfLetWithElse {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        let ExprKind::If(cond, _, Some(else_expr)) = expr.kind else {
            // r[impl lint.if-let-with-else.no-else-allowed]
            return;
        };

        // r[impl lint.if-let-with-else.detect]
        let ExprKind::Let(_) = cond.kind else {
            return;
        };

        // r[impl lint.if-let-with-else.diverging-else-ignored]
        if cx.typeck_results().expr_ty(else_expr).is_never() {
            return;
        }

        // r[impl lint.if-let-with-else.message]
        span_lint_and_help(
            cx,
            IF_LET_WITH_ELSE,
            expr.span,
            "`if let` with `else` should be written as a `match` expression",
            None,
            "use a `match` expression to make both arms equally visible",
        );
    }
}

#[test]
fn ui() {
    whisker_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
