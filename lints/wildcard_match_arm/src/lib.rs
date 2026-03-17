#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Arm, Expr, ExprKind, MatchSource, PatKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;

// r[impl lint.wildcard-match-arm.level]
dylint_linting::declare_late_lint! {
    /// ### What it does
    ///
    /// Flags wildcard (`_`) patterns in match arms when the scrutinee is an
    /// enum type.
    ///
    /// ### Why is this bad?
    ///
    /// Wildcard arms hide missing cases when new variants are added to an
    /// enum. Explicit matching forces you to handle each variant
    /// deliberately.
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
    /// match status {
    ///     Status::Active => {},
    ///     _ => {},
    /// }
    ///
    /// // Good:
    /// match status {
    ///     Status::Active => {},
    ///     Status::Inactive => {},
    ///     Status::Pending => {},
    /// }
    /// ```
    pub WILDCARD_MATCH_ARM,
    Warn,
    "wildcard match arms hide unhandled variants"
}

impl<'tcx> LateLintPass<'tcx> for WildcardMatchArm {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        // r[impl lint.wildcard-match-arm.detect]
        let ExprKind::Match(scrutinee, arms, MatchSource::Normal) = expr.kind else {
            return;
        };

        let scrutinee_ty = cx.typeck_results().expr_ty(scrutinee);

        // r[impl lint.wildcard-match-arm.non-enum-types]
        let ty::Adt(adt_def, _) = scrutinee_ty.kind() else {
            return;
        };
        if !adt_def.is_enum() {
            return;
        }

        // r[impl lint.wildcard-match-arm.non-exhaustive-external]
        // r[impl lint.wildcard-match-arm.non-exhaustive-local]
        if adt_def.variant_list_has_applicable_non_exhaustive() {
            return;
        }

        for Arm { pat, .. } in arms {
            if let PatKind::Wild = pat.kind {
                // r[impl lint.wildcard-match-arm.message]
                span_lint_and_help(
                    cx,
                    WILDCARD_MATCH_ARM,
                    pat.span,
                    "wildcard match arm hides unhandled variants",
                    None,
                    "match each variant explicitly instead of using `_`",
                );
            }
        }
    }
}

#[test]
fn ui() {
    whisker_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
