# Configuration

Whisker reads its configuration from `.config/whisker.toml`, at the top of your
project.

## A small file

Whisker ships no rules of its own, so a useful file names at least one source
of them. Each `[[lints]]` entry is one source. This file has two, a crate in
the repository and a repository of shared rules:

```toml title=".config/whisker.toml"
[[lints]]
path = "lints/doc_summary_break"

[[lints]]
git = "https://github.com/aonyx-ai/whisker-aonyx-rules"
rev = "0123456789abcdef0123456789abcdef01234567"
```

Every rule that either source provides now runs on `whisker check .`.

## Every setting

```toml title=".config/whisker.toml"
ignore = ["/examples/", "crates/whisker-rust/tests/fixtures/"]

[[lints]]
path = "lints/doc_summary_break"

[[lints]]
git = "https://github.com/aonyx-ai/whisker-aonyx-rules"
rev = "0123456789abcdef0123456789abcdef01234567"

[rules]
disable = ["lint.no-inline-comments"]

[rules.options."lint.repeated-primitive-params"]
boundary-attributes = ["shard", "procedure"]
```

Every key is optional, and a file that sets none of them is valid. `enable` is
the one setting missing above, because it and `disable` cannot both appear.

Whisker rejects a key it does not recognize, so a typo is an error rather than
a setting that silently does nothing.

## `ignore`

A list of gitignore-syntax patterns that exclude files from the run. Use it for
test fixtures, vendored sources, and other code that git tracks on purpose.

The patterns anchor at the project directory, the one that holds `.config`, and
they behave as they would in a `.gitignore` written there. `examples/` matches
at any depth, `/examples/` matches only at the top, and
`crates/app/generated/` names one directory. A trailing slash restricts a
pattern to directories, `**` crosses directory boundaries, and a leading `!`
re-includes an earlier exclusion. As in gitignore, `!` cannot bring back a file
whose parent directory is excluded, because the walk never enters that
directory.

These patterns come on top of what Whisker already skips. See
[checking a project][checking-a-project] for the rest.

Whisker compiles the patterns when the run starts, and invalid syntax is an
error that names the pattern.

## `[[lints]]`

Each entry names one source of rules. An entry names either a directory or a
repository, and naming both is an error. Every rule the source provides is
loaded; `[rules]` is what narrows them down.

A source may hold one Rust package or a cargo workspace of them. Whisker loads
every dynamic library the build produced, which is what lets one entry bring a
whole repository of rules. Writing such a crate is covered in
[custom lints][custom-lints].

### `path`

A directory holding a lint crate. A relative path anchors at the project
directory, the same one the `ignore` patterns anchor at. An absolute path
stands on its own.

```toml
[[lints]]
path = "lints/doc_summary_break"
```

### `git` and `rev`

A repository of rules, and the commit to take them from. Both keys are
required together. Pairing `rev` with `path` is an error.

```toml
[[lints]]
git = "https://github.com/aonyx-ai/whisker-aonyx-rules"
rev = "0123456789abcdef0123456789abcdef01234567"
```

`git` accepts what git accepts: an `https://` or `ssh://` remote, an scp-like
`git@host:org/repo`, and a `file://` or plain local path. Whisker reads the
remote when it reads the file, so a remote nothing can fetch fails at once
rather than minutes into a check.

`rev` is a full commit hash, 40 lowercase hexadecimal characters. A branch or a
tag names whatever the remote points it at today, and an abbreviated hash grows
ambiguous as a repository gains objects. Both are refused.

Whisker keeps the checkout under `~/.cache/whisker`, under `XDG_CACHE_HOME`
when that is set, or under `WHISKER_CACHE_DIR` when you set that. A git source
builds with `--locked`, so the repository of rules must commit its lockfile.
Before Whisker compiles anything, it asks the repository's releases for a
prebuilt archive. See [prebuilt lints][prebuilt-lints].

## `[rules]`

Which of the loaded rules run. `enable` and `disable` are two ways to say it,
and a file that names both is refused: a project that says which rules run has
already said which do not.

A name that no configured lint reports is an error too. A misspelled rule
disables nothing, and a run that skips it reads exactly like a rule that found
no fault.

### `enable`

Only the rules named run. This is how a project adopts Whisker one rule at a
time, and adds to the list as it fixes what each rule finds.

```toml
[rules]
enable = ["lint.doc-summary-break"]
```

### `disable`

Every rule runs except those named. This is the other direction: a project that
wants the whole set minus a few.

```toml
[rules]
disable = ["lint.no-inline-comments"]
```

## `[rules.options."<rule>"]`

What one rule cannot read off the source. Whether an attribute marks a system
boundary depends on which framework wrote the attribute, and no rule knows
every framework, so the project says:

```toml
[rules.options."lint.repeated-primitive-params"]
boundary-attributes = ["shard", "procedure"]
```

The table is keyed by rule id, and the rule must be one a configured lint
reports. Each value is a list of strings and nothing else, so a number or a
bare string is refused when the file is read.

Whisker never checks an option name, because nothing declares which options a
rule reads. An option a rule ignores is therefore silently ignored. Which
options a rule reads is that rule's own documentation.

## Where the file lives

The search starts at the path you check and climbs until it finds
`.config/whisker.toml`, or failing that, a `.git` directory. `whisker check
src/` still finds the configuration at the top of your repository, and a run
inside a repository never reads a file from outside it.

A directory with a configuration file of its own is its own project, so one
repository can hold several. A directory that is neither is still a valid
target: Whisker checks it and applies no patterns.

[checking-a-project]: /docs/reference/runs-and-outcomes
[custom-lints]: /authoring/how-to/write-a-rule
[prebuilt-lints]: /docs/reference/prebuilt-archives
