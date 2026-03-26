#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{FnDecl, Item, ItemKind, PrimTy, QPath, Ty, TyKind, intravisit::FnKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

// r[impl lint.bool-param.level]
dylint_linting::declare_late_lint! {
    /// ### What it does
    ///
    /// Flags `bool` parameters in function signatures and `bool` fields in
    /// struct definitions.
    ///
    /// ### Why is this bad?
    ///
    /// Boolean parameters and fields obscure intent at call sites and in data
    /// models. An enum with meaningful variant names makes the code
    /// self-documenting and prevents accidental transposition of arguments.
    ///
    /// ### Known problems
    ///
    /// None.
    ///
    /// ### Examples
    ///
    /// ```rust,ignore
    /// // Bad:
    /// fn create_repo(name: &str, is_public: bool) {}
    ///
    /// struct Config {
    ///     verbose: bool,
    /// }
    ///
    /// // Good:
    /// enum Visibility { Public, Private }
    /// fn create_repo(name: &str, visibility: Visibility) {}
    ///
    /// enum Verbosity { Quiet, Verbose }
    /// struct Config {
    ///     verbosity: Verbosity,
    /// }
    /// ```
    pub BOOL_PARAM,
    Warn,
    "bool parameters and fields obscure intent; use an enum with meaningful variants"
}

fn is_bool_ty(ty: &Ty<'_>) -> bool {
    let TyKind::Path(QPath::Resolved(None, path)) = ty.kind else {
        return false;
    };
    path.res == rustc_hir::def::Res::PrimTy(PrimTy::Bool)
}

impl<'tcx> LateLintPass<'tcx> for BoolParam {
    // r[impl lint.bool-param.detect-fn]
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        _body: &'tcx rustc_hir::Body<'tcx>,
        _span: Span,
        _def_id: rustc_hir::def_id::LocalDefId,
    ) {
        let (FnKind::ItemFn(..) | FnKind::Method(..)) = kind else {
            return;
        };

        for input in decl.inputs {
            if is_bool_ty(input) {
                // r[impl lint.bool-param.message]
                span_lint_and_help(
                    cx,
                    BOOL_PARAM,
                    input.span,
                    "parameter has type `bool`",
                    None,
                    "use an enum with meaningful variants instead of `bool`",
                );
            }
        }
    }

    // r[impl lint.bool-param.detect-struct]
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let ItemKind::Struct(_, _, variant_data) = &item.kind else {
            return;
        };

        for field in variant_data.fields() {
            if is_bool_ty(field.ty) {
                // r[impl lint.bool-param.message]
                span_lint_and_help(
                    cx,
                    BOOL_PARAM,
                    field.ty.span,
                    "struct field has type `bool`",
                    None,
                    "use an enum with meaningful variants instead of `bool`",
                );
            }
        }
    }
}

#[test]
fn ui() {
    whisker_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
