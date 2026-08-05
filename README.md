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

Whisker walks the target directory the way `git` and `ripgrep` do: anything
excluded by `.gitignore`, `.ignore`, `.git/info/exclude`, or your global
gitignore is skipped, as are hidden files and directories. Those files are
honored even outside a git checkout, since a `.gitignore` in an exported or
vendored tree still describes what that tree generates. A path named directly
on the command line is always checked, even if a rule would have excluded it,
and so is everything inside it unless a rule names something further down.
Naming a file whisker has no grammar for is an error, though, since calling it
clean would mean vouching for a file whisker never understood.

A run that finds nothing to check is an error rather than a success, because
a linter reporting no problems having opened no files is indistinguishable
from a clean project and far more likely to mean a pattern went too wide.

A directory that cannot be read, or an ignore file that cannot be parsed,
ends the run for the same reason: either one quietly changes which files get
looked at. Pass `--keep-going` to report them and carry on, which still exits
non-zero.

### Configuration

Whisker reads its configuration from the `[workspace.metadata.whisker]` table
of the target project's Cargo workspace manifest:

```toml
[workspace.metadata.whisker]
ignore = ["/examples/", "crates/whisker-rust/tests/fixtures/"]
```

`ignore` holds gitignore-syntax patterns, which is the right place for test
fixtures, vendored sources, and anything else that is deliberately not
idiomatic. The patterns behave exactly as they would in a `.gitignore` written
at the workspace root: `crates/app/generated/` names one directory relative to
that root, a bare `examples/` matches a directory of that name at any depth
beneath it, and `/examples/` anchors it to the root. Whisker rejects keys it
does not recognize, so a typo is an error rather than a setting that quietly
does nothing.

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
