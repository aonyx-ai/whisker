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

Whisker reads its configuration from `.config/whisker.toml` or
`.whisker.toml`:

```toml
ignore = ["/examples/", "crates/whisker-rust/tests/fixtures/"]
```

Either name works. A directory that holds both is an error, because whisker
cannot tell which one you meant. The search starts at the path you check and
climbs to the repository root. `whisker check src/` still finds the
configuration at the top of your repository.

`ignore` holds gitignore-syntax patterns. Use it for test fixtures, vendored
sources, and other code that git tracks on purpose. The patterns anchor at
the project directory: the directory that holds `.whisker.toml`, or the one
that holds `.config`. They behave as they would in a `.gitignore` written
there. `crates/app/generated/` names one directory relative to that root,
`examples/` matches at any depth, and `/examples/` matches only at the root.
Whisker rejects keys it does not recognize, so a typo is an error and not a
silent no-op.

### Custom lints

A project can bring its own rules. Each `lints` entry names a directory
holding a lint crate; relative paths anchor at the project directory, the
same way `ignore` patterns do:

```toml
[[lints]]
path = "lints/no_todo"
```

An entry can also name a repository, which is how a set of rules is shared
between projects. Whisker pins it to one commit, written out in full:

```toml
[[lints]]
git = "https://github.com/aonyx-ai/whisker-aonyx-rules"
rev = "0123456789abcdef0123456789abcdef01234567"
```

A branch or a tag is whatever the remote points it at today, so the same
configuration would run different rules on different days. Whisker asks for
that one commit and nothing else, keeps the checkout under
`~/.cache/whisker`, and reuses it forever after, because the commit it
names can never change. Set `WHISKER_CACHE_DIR` to keep those checkouts
somewhere else. A git source builds with `--locked`, so commit the
lockfile beside the rules.

`whisker check` compiles each entry with your `cargo`, loads the built
libraries, and runs their lints. Whisker ships no rules of its own, so the
rules a project configures are exactly the rules it runs. The first build takes
as long as any Rust compilation; afterwards cargo's cache makes it cheap.

A custom lint crate is a `cdylib` that implements `RustLintPass` and hands
its rules to `export_lints!`. The complete crate in
[`examples/custom_lint`][example] is the template: a `Cargo.toml` declaring
the crate type and the whisker dependencies, one rule, and its tests. An
entry may also name a directory holding a cargo workspace, in which case
every plugin in it loads, which is what lets one entry bring a whole
repository of rules.

Rust has no stable ABI, so whisker only loads a plugin built by the same
rustc from the same whisker source as the binary itself, and refuses
anything else with an error that says what to rebuild. In practice: pin the
plugin's whisker dependencies to the revision your whisker was built from,
and build both with the same toolchain.

That check covers whisker's own source and the compiler, not the rest of
the graph your plugin's lockfile resolves. Commit that lockfile and keep it
in step with the whisker you build against, the way
[`examples/custom_lint`][example] does.

[example]: examples/custom_lint

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
