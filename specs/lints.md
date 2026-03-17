# Lints

## Wildcard match arm

r[lint.wildcard-match-arm.detect]
The lint must flag wildcard (`_`) patterns in match arms when the scrutinee type
is an enum.

r[lint.wildcard-match-arm.non-enum-types]
The lint must not fire on non-enum types (integers, strings, booleans, etc.)
since these inherently require wildcard arms.

r[lint.wildcard-match-arm.non-exhaustive-external]
The lint must allow wildcard arms when the matched type is a `#[non_exhaustive]`
enum defined in an external crate.

r[lint.wildcard-match-arm.non-exhaustive-local]
The lint must flag wildcard arms on `#[non_exhaustive]` enums defined in the
current crate, since the author controls all variants.

r[lint.wildcard-match-arm.message]
The diagnostic must suggest matching each variant explicitly instead of using
`_`.

r[lint.wildcard-match-arm.level]
The lint must default to `Warn`.

## Anyhow missing context

r[lint.anyhow-missing-context.detect]
The lint must flag uses of the `?` operator on `Result` types where the
expression is not a `.context()` or `.with_context()` call.

r[lint.anyhow-missing-context.context-allowed]
The lint must not flag when `.context()` is called before `?`.

r[lint.anyhow-missing-context.with-context-allowed]
The lint must not flag when `.with_context()` is called before `?`.

r[lint.anyhow-missing-context.anyhow-only]
The lint must only flag `?` when the enclosing function returns
`Result<T, anyhow::Error>`. Functions returning other error types are
not flagged.

r[lint.anyhow-missing-context.option-ignored]
The lint must not flag `?` on `Option` types.

r[lint.anyhow-missing-context.message]
The diagnostic must suggest adding `.context("description")` before `?`.

r[lint.anyhow-missing-context.level]
The lint must default to `Warn`.

## No matches macro

r[lint.no-matches-macro.detect]
The lint must flag uses of the `matches!` macro regardless of the expression
context (let bindings, return positions, function arguments, conditions, etc.).
The lint must not flag regular `match` expressions or other macros such as
`assert!` or `println!`.

r[lint.no-matches-macro.message]
The diagnostic must suggest using a full `match` expression instead of the
`matches!` macro.

r[lint.no-matches-macro.level]
The lint must default to `Warn`.

## Bool param

r[lint.bool-param.detect-fn]
The lint must flag `bool` parameters in function signatures.

r[lint.bool-param.detect-struct]
The lint must flag `bool` fields in struct definitions.

r[lint.bool-param.return-type-allowed]
The lint must not flag `bool` return types.

r[lint.bool-param.local-var-allowed]
The lint must not flag `bool` local variables.

r[lint.bool-param.message]
The diagnostic must suggest using an enum with meaningful variants instead of
`bool`.

r[lint.bool-param.level]
The lint must default to `Warn`.

## Derive order

r[lint.derive-order.detect]
The lint must flag `#[derive(...)]` attributes whose derives are not in the
canonical order.

r[lint.derive-order.std-order]
Standard library derives must appear first and in the following order: Copy,
Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default. Only the derives
that are present need to appear, but their relative order must match this
sequence.

r[lint.derive-order.third-party-after-std]
Third-party derives (any derive not in the standard library list) must appear
after all standard library derives.

r[lint.derive-order.third-party-alpha]
Third-party derives must be sorted alphabetically by crate, then by macro name.

r[lint.derive-order.message]
The diagnostic must show the expected ordering of the derives.

r[lint.derive-order.level]
The lint must default to `Warn`.

## If let with else

r[lint.if-let-with-else.detect]
The lint must flag `if let` expressions that have an `else` branch where the
else branch does not diverge.

r[lint.if-let-with-else.diverging-else-ignored]
The lint must not flag `if let` expressions where the `else` branch diverges
(returns, panics, etc.). That case is handled by the `prefer_let_else` lint.

r[lint.if-let-with-else.no-else-allowed]
The lint must not flag `if let` expressions without an `else` branch.

r[lint.if-let-with-else.message]
The diagnostic must suggest using a `match` expression instead.

r[lint.if-let-with-else.level]
The lint must default to `Warn`.
