#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::token::TokenKind;
use rustc_ast::tokenstream::TokenTree;
use rustc_ast::{AttrArgs, AttrItemKind, AttrKind, Attribute, Item};
use rustc_lint::{EarlyContext, EarlyLintPass};
use rustc_span::Span;

const STD_DERIVES: &[&str] = &[
    "Copy",
    "Clone",
    "Eq",
    "PartialEq",
    "Ord",
    "PartialOrd",
    "Hash",
    "Debug",
    "Default",
];

// r[impl lint.derive-order.level]
dylint_linting::declare_pre_expansion_lint! {
    /// ### What it does
    ///
    /// Enforces a canonical ordering for `#[derive(...)]` attributes: standard
    /// library derives must appear first in the prescribed order (Copy, Clone,
    /// Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default), followed by
    /// third-party derives sorted alphabetically by crate and then by macro
    /// name.
    ///
    /// ### Why is this bad?
    ///
    /// Inconsistent derive ordering makes code harder to scan and review.
    /// A canonical ordering reduces cognitive load and eliminates style
    /// debates.
    ///
    /// ### Known problems
    ///
    /// None.
    ///
    /// ### Examples
    ///
    /// ```rust,ignore
    /// // Bad:
    /// #[derive(Debug, Clone, Default)]
    /// struct Foo;
    ///
    /// // Good:
    /// #[derive(Clone, Debug, Default)]
    /// struct Foo;
    /// ```
    pub DERIVE_ORDER,
    Warn,
    "derive macros are not in canonical order"
}

fn std_derive_index(name: &str) -> Option<usize> {
    STD_DERIVES.iter().position(|&s| s == name)
}

fn macro_name(full_path: &str) -> &str {
    full_path.rsplit("::").next().unwrap_or(full_path)
}

fn extract_derive_names(attrs: &[Attribute]) -> Option<(Vec<String>, Span)> {
    for attr in attrs {
        let AttrKind::Normal(normal_attr) = &attr.kind else {
            continue;
        };
        let has_derive = normal_attr
            .item
            .path
            .segments
            .iter()
            .any(|seg| seg.ident.name.as_str() == "derive");
        if !has_derive {
            continue;
        }
        let AttrItemKind::Unparsed(AttrArgs::Delimited(delim_args)) = &normal_attr.item.args else {
            continue;
        };

        let mut names = Vec::new();
        let mut current_path = String::new();

        for tt in delim_args.tokens.iter() {
            let TokenTree::Token(token, _) = tt else {
                continue;
            };
            if let TokenKind::Ident(sym, _) = token.kind {
                if !current_path.is_empty() {
                    current_path.push_str("::");
                }
                current_path.push_str(sym.as_str());
            }
            if let TokenKind::Comma = token.kind
                && !current_path.is_empty()
            {
                names.push(std::mem::take(&mut current_path));
            }
        }
        if !current_path.is_empty() {
            names.push(current_path);
        }

        if !names.is_empty() {
            return Some((names, attr.span));
        }
    }
    None
}

fn compute_expected_order(names: &[String]) -> Vec<String> {
    let mut std_derives: Vec<&String> = names
        .iter()
        .filter(|n| std_derive_index(macro_name(n)).is_some())
        .collect();
    std_derives.sort_by_key(|n| std_derive_index(macro_name(n)).unwrap_or(usize::MAX));

    let mut third_party: Vec<&String> = names
        .iter()
        .filter(|n| std_derive_index(macro_name(n)).is_none())
        .collect();
    third_party.sort_by_key(|a| a.to_lowercase());

    let mut result: Vec<String> = std_derives.into_iter().cloned().collect();
    result.extend(third_party.into_iter().cloned());
    result
}

// r[impl lint.derive-order.detect]
impl EarlyLintPass for DeriveOrder {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        let Some((names, span)) = extract_derive_names(&item.attrs) else {
            return;
        };

        if names.len() <= 1 {
            return;
        }

        let expected = compute_expected_order(&names);

        if names == expected {
            return;
        }

        // r[impl lint.derive-order.message]
        let expected_str = expected.join(", ");
        span_lint_and_then(
            cx,
            DERIVE_ORDER,
            span,
            "derive macros are not in canonical order",
            |diag| {
                diag.help(format!("expected order: {expected_str}"));
            },
        );
    }
}

#[test]
fn ui() {
    whisker_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
