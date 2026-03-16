# Lints

## Wildcard match arm

r[lint.wildcard-match-arm.detect]
The lint must flag wildcard (`_`) patterns in match arms of `match` expressions.

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
