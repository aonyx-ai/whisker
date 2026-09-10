# Command line

## `whisker check`

```bash
whisker check [OPTIONS] [PATH]
```

| Argument          | Meaning                                                        |
| ----------------- | -------------------------------------------------------------- |
| `PATH`            | The directory or file to check. Defaults to `.`                |
| `--keep-going`    | Report each failure and continue. The run still exits non-zero |
| `--deny-warnings` | Fail on warnings as well as errors                             |

`PATH` decides which project Whisker reads its configuration from, and which
Cargo workspace rust-analyzer loads. See
[the configuration file][configuration].

## `whisker abi`

```bash
whisker abi
```

Prints the tag that names the prebuilt libraries this binary loads, such as
`a1b2c3d4-aarch64-apple-darwin`. Publishers put it in an archive name. See
[prebuilt archives][prebuilt-lints].

## Global options

| Option            | Meaning                         |
| ----------------- | ------------------------------- |
| `-h`, `--help`    | Print help                      |
| `-V`, `--version` | Print the version               |
| `-q`, `--quiet`   | Suppress informational messages |
| `-v`, `--verbose` | Show additional detail          |
| `--json`          | Output results as JSON          |

Diagnostics and `help:` lines always render as text on stderr. The three output
flags come from the CLI framework and do not currently change them.

## Exit status

| Status | Meaning                                                                         |
| ------ | ------------------------------------------------------------------------------- |
| `0`    | Nothing failed the run                                                          |
| `1`    | A diagnostic met the failure threshold, or a file could not be read or analyzed |

[Runs and outcomes][checking-a-project] covers what reaches each status.

[checking-a-project]: /docs/reference/runs-and-outcomes
[configuration]: /docs/reference/configuration
[prebuilt-lints]: /docs/reference/prebuilt-archives
