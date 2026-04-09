# If let with else

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
