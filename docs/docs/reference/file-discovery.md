# File discovery

<!--
goal: answer "why was this file skipped" and "why was that one checked".
non-goal: the ignore pattern syntax, which configuration#ignore owns.
-->

Whisker has a fairly simple algorithm to discover which files to lint. However,
sometimes it may include or skip a file unexpectedly. Refer to this page to find
out how Whisker discovers files.

## Included files

Whisker walks through the files or directories you list in its `check`
arguments. Every file with an extension supported by Whisker is checked.
Anything else is always ignored.

| Extension                     | Language   | Project file    |
| ----------------------------- | ---------- | --------------- |
| `.rs`                         | Rust       | `Cargo.toml`    |
| `.ts`, `.mts`, `.cts`, `.tsx` | TypeScript | `tsconfig.json` |

Whisker loads a language's project only when it finds a file in that language,
so a repository with no TypeScript needs no `tsconfig.json`. A TypeScript file
belongs to the project of the `tsconfig.json` nearest it, so one repository can
hold several TypeScript projects.

If it turns out that an included file cannot be parsed by one of Whisker's
language adapters, Whisker will throw an error.

## Ignored files

There are several reasons Whisker might ignore a file:

| Skipped                      | Source                 |
| ---------------------------- | ---------------------- |
| Hidden files and directories | The walk               |
| `./.gitignore`               | The repository         |
| `./.ignore`                  | The repository         |
| `./.git/info/exclude`        | The repository         |
| global gitignore             | Your git configuration |
| `ignore` patterns            | `.config/whisker.toml` |

## Checking specific files

You can override the ignore rules above by passing a specific file to
`whisker check`.
