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
