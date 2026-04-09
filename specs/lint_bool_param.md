# Bool param

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
