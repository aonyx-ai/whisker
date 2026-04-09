# No matches macro

r[lint.no-matches-macro.detect]
The lint must flag uses of the `matches!` macro regardless of the expression
context (let bindings, return positions, function arguments, conditions, etc.).
The lint must not flag regular `match` expressions or other macros such as
`assert!` or `println!`.

r[lint.no-matches-macro.message]
The diagnostic must suggest using a full `match` expression instead of the
`matches!` macro.
