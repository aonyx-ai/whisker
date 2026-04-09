# Anyhow missing context

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
