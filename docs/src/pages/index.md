# Whisker

Whisker is a linting platform built on [tree-sitter][ts]. It lints Rust today.

Whisker ships no rules of its own. A project configures the ones it wants, and
those are exactly the ones it runs. Aonyx's own live in
[whisker-aonyx-rules][rules] and cover what Clippy does not, like derive
ordering, wildcard match arms, `matches!` macro usage, and the other style
rules defined in our `CLAUDE.md` files.

A rule implements the `RustLintPass` trait, which whisker generates from
tree-sitter's Rust grammar. A rule that needs type information reads
decorations that whisker computes with [rust-analyzer][ra] before any rule
runs.

## Status

Whisker is in early development. Check back soon.

## Where to start

- **[Quick start](/docs/quick-start)** — install whisker, point it at a set of
  rules, and check a project.
- **[Configuration](/docs/configuration)** — what `.config/whisker.toml` holds
  and where whisker looks for it.
- **[Custom lints](/docs/custom-lints)** — write your own rules, or run someone
  else's.

[ra]: https://rust-analyzer.github.io/
[rules]: https://github.com/aonyx-ai/whisker-aonyx-rules
[ts]: https://tree-sitter.github.io/tree-sitter/
