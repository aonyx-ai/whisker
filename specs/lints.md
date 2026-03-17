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
