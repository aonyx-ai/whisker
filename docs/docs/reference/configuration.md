# Configuration

<!--
goal: answer "what can I put in this file, and what does each key do".
non-goal: persuading anyone to adopt a setting. the how-to guides do that.
-->

Whisker requires a configuration file to be useful. Here we explain the various
options that are available for configuration.

## Basic Whisker config

The most important thing to configure is including rules to be used:

```toml title=".config/whisker.toml"
[[lints]]
path = "lints/doc_summary_break"

[[lints]]
git = "https://github.com/aonyx-ai/whisker-aonyx-rules"
rev = "0123456789abcdef0123456789abcdef01234567"
```

## All options

This is an overview of all available options. Each configuration is explained
below.

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

## ignore {#ignore}

To ignore specific files, you can use the top-level `ignore` property. You can
ignore specific files, entire directories, or through gitignore-like patterns.

```toml title=".config/whisker.toml"
ignore = [
  # ignore the top-level examples/ directory
  "/examples/",

  # ignore any fixtures/ directory at any depth
  "fixtures/",

  # ignore all .rs files directly under the /crate/foo/ directory
  "/crate/foo/*.rs",

  # ignore all .rs files in the /crate/foo/ tree, including all nested subdirectories
  "/crate/bar/**.rs",

  # don't ignore this specific file, regardless of other ignore rules
  "!/crate/bar/main.rs",
]
```

## [[lints]]

You can load multiple lints, each is defined through a `[[lints]]` section. To
define the lints, you either list a path or a git repository.

### path

Use a lint at a specific directory, relative to the project root. This directory
has to be a Whisker lint, as described in [writing a rule][writing-a-rule].

```toml
[[lints]]
path = "lints/doc_summary_break"
```

### git and rev

Use a lint at a (remote) Git repository. You must define a full 40-character
hash. Learn more at [Shared rules][shared-rules].

```toml
[[lints]]
git = "https://github.com/aonyx-ai/whisker-aonyx-rules"
rev = "0123456789abcdef0123456789abcdef01234567"
```

### [rules]

These are rule-specific configurations that Whisker can keep itself to.

#### enable

To enable only specific rules in a lint, add an `enable` property to the
`[rules]` section. Learn more in [Adopt rules gradually][adopt-rules-gradually].

```toml
[rules]
enable = ["lint.doc-summary-break"]
```

#### disable

To disable only specific rules in a lint, add an `enable` property to the
`[rules]` section. Learn more in [Adopt rules gradually][adopt-rules-gradually].

```toml
[rules]
disable = ["lint.no-inline-comments"]
```

#### rules.options {#rule-options}

Whisker rules may define more granular options. These will be documented in the
rule's documentation. To use these options, add them under a
`[rules.options."rule-name"]` section in the rule's configuration.

```toml
[rules.options."lint.repeated-primitive-params"]
boundary-attributes = ["shard", "procedure"]
```

## whisker-source

For Whisker's own development we need to patch `whisker-types`, `whisker-rust`,
and `whisker-testing`. Lints will be built against these libraries instead of
their crates.io versions. This utilizes the `whisker-source` property. You
probably shouldn't need this option. define

```toml
whisker-source = "."
```

## Config file location

In your Git repository root, create a file called `.config/whisker.toml`.
Whisker will look for the first parent directory containing this path, but stops
looking at a directory containing a `.git` directory.

[adopt-rules-gradually]: /docs/how-to/adopt-rules-gradually
[shared-rules]: /docs/how-to/shared-rules
[writing-a-rule]: /authoring/how-to/writing-a-rule
