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

## Prefer let-else

r[lint.prefer-let-else.detect]
The lint must flag `if let` expressions where the `else` branch diverges
(returns, breaks, continues, or calls a diverging function such as `panic!`).

r[lint.prefer-let-else.if-let-only]
The lint must not fire on regular `if-else` expressions (without a `let`
pattern), or on `if let` expressions that have no `else` branch.

r[lint.prefer-let-else.diverging-else]
The lint must not fire when the `else` branch does not diverge (i.e., when the
`else` branch has a non-`!` return type).

r[lint.prefer-let-else.message]
The diagnostic must suggest rewriting the `if let` as `let-else` syntax.

r[lint.prefer-let-else.level]
The lint must default to `Warn`.
