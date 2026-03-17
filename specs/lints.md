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
