---
sidebar_position: 4
---

# Custom lints

A project can bring its own rules. Each `lints` entry names a directory that
holds a lint crate. Relative paths anchor at the project directory, the same
way `ignore` patterns do:

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
that one commit and nothing else. It keeps the checkout under
`~/.cache/whisker`, or under `XDG_CACHE_HOME` when that is set. A commit hash
names one immutable tree, so whisker reuses the checkout on every later run.
Set `WHISKER_CACHE_DIR` to keep those checkouts somewhere else. A git source
builds with `--locked`, so commit the lockfile beside the rules.

`whisker check` compiles each entry with your `cargo`, loads the built
libraries, and runs their lints. Whisker ships no rules of its own, so the
rules a project configures are exactly the rules it runs. The first build takes
as long as any Rust compilation; afterwards cargo's cache makes it cheap.

A repository can publish its rules already compiled, which skips that build.
See [prebuilt lints](/docs/prebuilt-lints).

## Choosing rules

A `[[lints]]` entry brings every rule its source provides. Name the ones you
want, or the ones you do not:

```toml
[rules]
disable = ["lint.no-inline-comments"]
```

`enable` is the other half: name it and only those rules run, which is how a
project adopts a rule at a time. Naming both is refused. So is naming a rule
that no configured lint reports, because a misspelling would otherwise disable
nothing and read exactly like a rule that found no fault.

## Writing a lint crate

A custom lint crate is a `cdylib` that implements `RustLintPass` and hands its
rules to `export_lints!`. The complete crate in [`examples/custom_lint`][example]
is the template: a `Cargo.toml` declaring the crate type and the whisker
dependencies, one rule, and its tests. An entry may also name a directory that
holds a cargo workspace. Every plugin in it loads, which is what lets one entry
bring a whole repository of rules.

## The plugin boundary

Rust has no stable ABI, so whisker only loads a plugin built by the same rustc
from the same whisker source as the binary itself, and refuses anything else
with an error that says what to rebuild. In practice: pin the plugin's whisker
dependencies to the revision your whisker was built from, and build both with
the same toolchain.

A released whisker accepts only a library built by the nightly that built it.
To compile a lint crate for one yourself, install the toolchain that
`rust-toolchain.toml` names at the release's commit. Otherwise build whisker
from source and compile the lint crate with the same toolchain.

That check covers whisker's own source and the compiler. The rest of the graph
your plugin's lockfile resolves lies outside it. Commit that lockfile and keep
it in step with the whisker you build against, the way
[`examples/custom_lint`][example] does.

[example]: https://github.com/aonyx-ai/whisker/tree/main/examples/custom_lint
