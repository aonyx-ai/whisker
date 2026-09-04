# Whisker

Whisker is a linting platform built on [tree-sitter][ts]. It lints Rust
today. Whisker ships no rules of its own: a project configures the ones it
wants, and those are exactly the ones it runs. Aonyx's own live in
[whisker-aonyx-rules][rules] and cover what Clippy does not, like derive
ordering, wildcard match arms, `matches!` macro usage, and the other style
rules defined in our `CLAUDE.md` files.

A rule implements the `RustLintPass` trait, which whisker generates from
tree-sitter's Rust grammar. A rule that needs type information reads
decorations that whisker computes with [rust-analyzer][ra] before any rule
runs.

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

The Linux binaries need glibc 2.35 or newer, which Ubuntu 22.04 and Debian
12 satisfy. Build from source on anything older.

On GitHub Actions, the action in this repository does the same three
steps, for the runner it finds itself on:

```yaml
- uses: aonyx-ai/whisker@v0.1.0-rc.2
  with:
    version: v0.1.0-rc.2
- run: whisker check .
```

Whisker also builds from source. `rust-toolchain.toml` pins a nightly
toolchain, and rustup installs it during the build:

```bash
cargo install --git https://github.com/aonyx-ai/whisker --locked whisker
```

The choice decides how whisker obtains your custom lints. See
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

`whisker check` needs a Cargo project. Whisker uses rust-analyzer to load
the workspace nearest the path you name, and it runs that workspace's
build scripts before it lints anything. A file that no crate in the workspace
reaches has no type information. Whisker reports it as an error and prints
what to do about it. The same happens to a file that rust-analyzer
excludes from the workspace.

A directory that whisker cannot read, or an ignore file that it cannot
parse, ends the run: each one changes which files whisker inspects. A file
that whisker cannot read or analyze ends the run too. Pass `--keep-going`
to report each failure, continue, and still exit non-zero. A diagnostic at
the error severity also fails the run. Pass `--deny-warnings` to fail on
a warning too.

A run that finds nothing to check is an error. An empty run
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
recognize, so a typo is an error.

### Custom lints

A project can bring its own rules. Each `lints` entry names a directory
that holds a lint crate. Relative paths anchor at the project directory,
the same way `ignore` patterns do:

```toml
[[lints]]
path = "lints/todo_comment"
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
that one commit and nothing else. It keeps the checkout under
`~/.cache/whisker`, or under `XDG_CACHE_HOME` when that is set. A commit
hash names one immutable tree, so whisker reuses the checkout on every
later run. Set `WHISKER_CACHE_DIR` to keep those checkouts somewhere else.
A git source builds with `--locked`, so commit the lockfile beside the
rules.

`whisker check` compiles each entry with your `cargo`, loads the built
libraries, and runs their lints. Whisker ships no rules of its own, so the
rules a project configures are exactly the rules it runs. The first build takes
as long as any Rust compilation; afterwards cargo's cache makes it cheap.

#### Naming a rule

A rule id names the thing the rule reports. `todo_comment` reports a TODO
comment, and `repeated_field_access` reports a statement that reads two or
more fields of one binding. The id never names the state the author should
reach instead, and never negates.

Negation is the part that matters beyond consistency. A rule id is what a
project names to turn the rule off, and a negated id inverts under that:
`disable = ["lint.no-todo-comments"]` reads as turning off the absence of
TODO comments, which is the opposite of what it does. Clippy names every
lint after the thing it reports for the same reason.

Keep the id singular, unless the finding needs more than one of something
to exist. `missing_` is not negation: when a rule reports an absence, the
absence is the thing it reports, so `missing_trait_tests` follows the
convention as it stands.

The crate directory, the package name, the `RULE_ID` constant, and the
lint pass type all spell one name: `todo_comment`, `todo_comment`,
`lint.todo-comment`, and `TodoComment`.

#### Prebuilt lints

A repository can publish its rules already compiled. Before whisker
fetches a git entry, it asks that repository's releases for an archive.
The archive is named
after the pinned commit and after the tag that `whisker abi` prints. If a
release carries that archive and the `.sha256` beside it, whisker
downloads it and checks the digest. It then unpacks the archive into the
cache and loads the libraries. Each library still completes the handshake
described below, and one that fails it ends the run.

Whisker asks a release only when the cache holds nothing for the entry.
Libraries it unpacked before come first, then a checkout it compiled
before. A project with a cached checkout therefore keeps compiling it
after its rules start to publish archives. Move the pin or clear the
cache to pick the archives up.

A repository that publishes nothing for your tag is the ordinary case,
and whisker compiles the source and says nothing. A download that fails
or a digest that does not match prints one line on stderr, and whisker
compiles the source.

Set `GH_TOKEN` or `GITHUB_TOKEN` to reach a private repository, and
`WHISKER_GITHUB_API_URL` to point whisker at a GitHub Enterprise
installation. Whisker sends the token only to that API.

The digest proves that the download arrived intact. The same publisher
writes the archive and the digest, so the digest establishes no trust in
the publisher. A repository you configure runs its code in whisker's
process, prebuilt or compiled.

A released whisker accepts only a library built by the nightly that
built it. To compile a lint crate for one yourself, install the toolchain
that `rust-toolchain.toml` names at the release's commit. Otherwise build
whisker from source and compile the lint crate with the same toolchain.

A `[[lints]]` entry brings every rule its source provides. Name the ones
you want, or the ones you do not:

```toml
[rules]
disable = ["lint.no-inline-comments"]
```

`enable` is the other half: name it and only those rules run, which is how
a project adopts a rule at a time. Naming both is refused. So is naming a
rule that no configured lint reports, because a misspelling would
otherwise disable nothing and read exactly like a rule that found no
fault.

A custom lint crate is a `cdylib` that implements `RustLintPass` and hands
its rules to `export_lints!`. The complete crate in
[`examples/custom_lint`][example] is the template: a `Cargo.toml` declaring
the crate type and the whisker dependencies, one rule, and its tests. An
entry may also name a directory that holds a cargo workspace. Every
plugin in it loads, which is what lets one entry bring a whole repository
of rules.

Rust has no stable ABI, so whisker only loads a plugin built by the same
rustc from the same whisker source as the binary itself, and refuses
anything else with an error that says what to rebuild. In practice: pin the
plugin's whisker dependencies to the revision your whisker was built from,
and build both with the same toolchain.

That check covers whisker's own source and the compiler. The rest of the
graph your plugin's lockfile resolves lies outside it. Commit that
lockfile and keep it in step with the whisker you build against, the way
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
