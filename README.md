# Whisker

Whisker is a language-agnostic linting platform built on [tree-sitter][ts].
It ships with Rust lints that enforce Aonyx's coding conventions — patterns
that Clippy doesn't cover, like derive ordering, wildcard match arms,
`matches!` macro usage, and other style rules defined in our `CLAUDE.md`
files.

Each lint is a separate crate in the `lints/` directory, implementing the
`RustLintPass` trait generated from tree-sitter's Rust grammar. Type-dependent
lints use [rust-analyzer][ra] as a library for semantic analysis.

## Status

Whisker is in early development. Check back soon.

## Usage

```bash
whisker check .
```

Whisker walks the target directory the way `git` and `ripgrep` do. It skips
hidden files and directories, and it skips anything that `.gitignore`,
`.ignore`, `.git/info/exclude`, or your global gitignore excludes. These
rules apply even outside a git checkout, because an ignore file in an
exported or vendored tree still describes what that tree generates. Whisker
always checks a path you name on the command line, even when an ignore rule
matches that path. Ignore rules still apply to files below a named
directory.

A directory that whisker cannot read, or an ignore file that it cannot
parse, ends the run: each one changes which files whisker inspects. A file
that whisker cannot read or analyze ends the run too. Pass `--keep-going`
to report each failure, continue, and still exit non-zero.

A run that finds nothing to check is an error, not a success. An empty run
usually means a pattern matched too much, and it would otherwise look like
a clean project.

Whisker refuses a named file that it has no grammar for. A parse with the
wrong grammar finds nothing and would report the file clean.

### Configuration

Whisker reads its configuration from the `[workspace.metadata.whisker]` table
of the target workspace manifest:

```toml
[workspace.metadata.whisker]
ignore = ["/examples/", "crates/whisker-rust/tests/fixtures/"]
```

`ignore` holds gitignore-syntax patterns. Use it for test fixtures, vendored
sources, and other code that git tracks on purpose. The patterns behave as
they would in a `.gitignore` at the workspace root: `crates/app/generated/`
names one directory relative to that root, `examples/` matches at any depth,
and `/examples/` anchors to the root. Whisker rejects keys it does not
recognize, so a typo is an error and not a silent no-op.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)
  or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT)
  or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[ra]: https://rust-analyzer.github.io/
[ts]: https://tree-sitter.github.io/tree-sitter/
