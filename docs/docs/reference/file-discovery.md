# File discovery

Whisker walks the target the way `git` and `ripgrep` do, then applies the
project's own patterns.

## What the walk skips

| Skipped                                 | Source                 |
| --------------------------------------- | ---------------------- |
| Hidden files and directories            | The walk               |
| Anything a `.gitignore` excludes        | The repository         |
| Anything an `.ignore` excludes          | The repository         |
| Anything `.git/info/exclude` excludes   | The repository         |
| Anything your global gitignore excludes | Your git configuration |
| Anything an `ignore` pattern excludes   | `.config/whisker.toml` |

These rules apply outside a git checkout too. An ignore file in an exported or
vendored tree still describes what that tree generates.

## Paths you name

Whisker always checks a path you name on the command line, even when an ignore
rule matches it. Ignore rules still apply to files below a named directory.

Whisker refuses a named file it has no grammar for. A parse with the wrong
grammar finds nothing and would report the file clean.

## Which files a rule sees

A file's extension decides whether discovery collects it. Rust support covers
`.rs`.

Discovery is not the whole answer to what gets linted: a file that no
decoration provider covers is an error rather than a file checked with
syntactic rules only. See [runs and outcomes][checking-a-project] and
[why a run refuses what it cannot see][coverage].

The `ignore` patterns are documented in [the configuration file][configuration].

[checking-a-project]: /docs/reference/runs-and-outcomes
[configuration]: /docs/reference/configuration#ignore
[coverage]: /docs/explanation/coverage
