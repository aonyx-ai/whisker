# Whisker

Whisker is a language-agnostic linting platform built on [tree-sitter][ts].
It ships no rules of its own: a project configures the ones it wants, and
those are exactly the ones it runs. Aonyx's own live in
[whisker-aonyx-rules][rules] and cover what Clippy does not, like derive
ordering, wildcard match arms, `matches!` macro usage, and the other style
rules defined in our `CLAUDE.md` files.

Each rule is a separate crate implementing the `RustLintPass` trait generated
from tree-sitter's Rust grammar. Type-dependent rules use
[rust-analyzer][ra] as a library for semantic analysis.

## Status

Whisker is in early development. Check back soon.

## Installation

Every release carries an archive for Linux on x86-64 and arm64, and for
macOS on Apple silicon. Download the one for your platform from the
[releases page][releases], check it against the `.sha256` beside it, and
unpack it:

```bash
shasum -a 256 -c whisker-0.1.0-aarch64-apple-darwin.tar.gz.sha256
tar -xzf whisker-0.1.0-aarch64-apple-darwin.tar.gz
```

The archive holds the binary, both licenses, and this README. Move
`whisker` to a directory on your `PATH`, such as `~/.local/bin`.

Whisker also builds from source. It pins a nightly toolchain in
`rust-toolchain.toml`, which rustup installs for you:

```bash
cargo install --git https://github.com/aonyx-ai/whisker --locked whisker
```

Which of the two you use decides how your custom lints are obtained. See
[custom lints](#custom-lints).

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

Whisker reads its configuration from `.config/whisker.toml`:

```toml
ignore = ["/examples/", "crates/whisker-rust/tests/fixtures/"]
```

The search starts at the path you check and climbs until it finds that file,
or failing that, a `.git` directory. `whisker check src/` still finds the
configuration at the top of your repository, and a run inside a repository
never reads a file from outside it. A directory with a configuration file of
its own is its own project, so one repository can hold several. A directory
that is neither is still a valid target: whisker checks it and applies no
patterns.

`ignore` holds gitignore-syntax patterns. Use it for test fixtures, vendored
sources, and other code that git tracks on purpose. The patterns anchor at
the project directory, the one that holds `.config`, and they behave as they
would in a `.gitignore` written there. `crates/app/generated/` names one
directory relative to that root, `examples/` matches at any depth, and
`/examples/` matches only at the root. Whisker rejects keys it does not
recognize, so a typo is an error and not a silent no-op.

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

#### Prebuilt lints

A repository can publish its rules already compiled, and whisker prefers
those to compiling them itself. Before it builds a git entry, whisker asks
that repository's releases for an archive named after the pinned commit
and after this binary's own tag, which `whisker abi` prints:

```bash
whisker abi
```

If a release carries that archive and the `.sha256` beside it, whisker
downloads it, checks the digest, unpacks it into its cache, and loads the
libraries. The cache is `WHISKER_CACHE_DIR` when you set it, and
`~/.cache/whisker` otherwise. A library that arrives this way still
completes the same handshake described below, and one that fails it ends
the run.

Whisker asks only when it holds nothing better. Libraries it already
unpacked come first, and a checkout already in the cache comes next.
Cargo compiled that checkout before, and a run that needs no network
should not make one. A project with a warm checkout therefore keeps
compiling it, even after its rules start to publish archives. Move the
pin or clear the cache to pick those up.

Everything else falls back to a source build, which is what whisker did
before any of this existed. A repository that publishes nothing for your
tag is the ordinary case and whisker says nothing about it; a download
that fails or a digest that does not match earns one line on stderr.

Set `GH_TOKEN` or `GITHUB_TOKEN` to reach a private repository, and
`WHISKER_GITHUB_API_URL` to point whisker at a GitHub Enterprise
installation. Whisker sends a token only to the API it was pointed at.

The digest guards against a download that arrived truncated or corrupted.
It is published by whoever publishes the archive, so it says nothing about
whether that archive deserves your trust. Configuring a repository of
lints is already a decision to run its code.

This is also why a whisker you downloaded and a lint crate you compile
yourself rarely fit. The handshake below accepts a plugin only from the
rustc that built whisker, and a released binary was built with the pinned
nightly. Either install that toolchain, or build whisker from source.

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
[releases]: https://github.com/aonyx-ai/whisker/releases

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
[rules]: https://github.com/aonyx-ai/whisker-aonyx-rules
[ts]: https://tree-sitter.github.io/tree-sitter/
