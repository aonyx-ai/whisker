# Command line

<!--
goal: let a reader look up a flag, an exit code, or what a run does when
something fails.
non-goal: the design reasoning behind the strictness. how-whisker-works carries
that.
-->

The main way users interact with Whisker is through our CLI. We describe the
various options here.

## check

Normally you'll invoke Whisker through `whisker check .`. There's some more
options you can pass though:

```bash
whisker check [OPTIONS] [PATH] [ARGS]...
```

| Argument          | Meaning                                                        |
| ----------------- | -------------------------------------------------------------- |
| `PATH`            | The directory or file to check. Defaults to `.`                |
| `ARGS`            | Accepted and discarded                                         |
| `--keep-going`    | Report each failure and continue. The run still exits non-zero |
| `--deny-warnings` | Fail on warnings as well as errors                             |

## abi

To find the ABI version of a Whisker binary, you can run this command. It takes
no options.

```bash
whisker abi
```

## Global options

With every Whisker command, you can use the following parameters to modify its
output or behavior:

| Option            | Meaning                         |
| ----------------- | ------------------------------- |
| `-h`, `--help`    | Print help                      |
| `-V`, `--version` | Print the version               |
| `-q`, `--quiet`   | Suppress informational messages |
| `-v`, `--verbose` | Show additional detail          |
| `--json`          | Output results as JSON          |

## What ends a run early

<!--
why does whisker stop at the first failure. the causes below change which files
get inspected, so a report that carried on would cover a different set.
how do I see every failure at once. --keep-going reports each one and carries
on, and the run still exits 1.
why is an empty run the exception. the check for it runs before the first file
and never consults --keep-going.
-->

| Cause                                 | Default      | With `--keep-going`     |
| ------------------------------------- | ------------ | ----------------------- |
| A directory Whisker cannot read       | Ends the run | Reported, run continues |
| An ignore file Whisker cannot parse   | Ends the run | Reported, run continues |
| A file Whisker cannot read or analyze | Ends the run | Reported, run continues |
| A rule that panics                    | Ends the run | Reported, run continues |
| A run that finds nothing to check     | Ends the run | Ends the run            |

## Exit status

Whisker exits with one of two statuses:

| Status | Meaning                                                                         |
| ------ | ------------------------------------------------------------------------------- |
| `0`    | Nothing failed the run                                                          |
| `1`    | A diagnostic met the failure threshold, or a file could not be read or analyzed |

<!--
which severities count. the table with and without --deny-warnings.
will the output match the exit code. yes, a promoted warning renders as an
error.
-->

| Severity | Fails by default | Fails with `--deny-warnings` |
| -------- | ---------------- | ---------------------------- |
| `Error`  | Yes              | Yes                          |
| `Warn`   | No               | Yes                          |
| `Info`   | No               | No                           |
| `Help`   | No               | No                           |

## Where output goes

<!--
stdout or stderr. diagnostics, errors, notes and help lines all go to stderr.
whisker abi is the exception and prints its tag to stdout, which is what makes
it usable in a shell substitution.
-->
