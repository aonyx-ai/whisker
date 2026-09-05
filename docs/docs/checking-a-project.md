---
sidebar_position: 2
---

# Checking a project

```bash
whisker check .
```

## Which files whisker inspects

Whisker walks the target directory the way `git` and `ripgrep` do. It skips
hidden files and directories, and it skips anything that `.gitignore`,
`.ignore`, `.git/info/exclude`, or your global gitignore excludes. These rules
apply even outside a git checkout, because an ignore file in an exported or
vendored tree still describes what that tree generates. Whisker always checks a
path you name on the command line, even when an ignore rule matches that path.
Ignore rules still apply to files below a named directory.

## Whisker needs a Cargo project

`whisker check` needs a Cargo project. Whisker uses rust-analyzer to load the
workspace nearest the path you name, and it runs that workspace's build scripts
before it lints anything. A file that no crate in the workspace reaches has no
type information. Whisker reports it as an error and prints what to do about
it. The same happens to a file that rust-analyzer excludes from the workspace.

## What makes a run fail

A directory that whisker cannot read, or an ignore file that it cannot parse,
ends the run: each one changes which files whisker inspects. A file that
whisker cannot read or analyze ends the run too. Pass `--keep-going` to report
each failure, continue, and still exit non-zero. A diagnostic at the error
severity also fails the run. Pass `--deny-warnings` to fail on a warning too.

A run that finds nothing to check is an error. An empty run usually means a
pattern matched too much, and it would otherwise look like a clean project.

Whisker refuses a named file that it has no grammar for. A parse with the wrong
grammar finds nothing and would report the file clean.
