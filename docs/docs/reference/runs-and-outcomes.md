# Runs and outcomes

```bash
whisker check .
```

## Whisker needs a Cargo project

Whisker uses rust-analyzer to load the workspace nearest the path you name, and
runs that workspace's build scripts before it lints anything.

A file that no crate in the workspace reaches has no type information. Whisker
reports it as an error and prints what to do about it. The same happens to a
file that rust-analyzer excludes from the workspace.

## What ends a run

| Cause                                 | Effect       |
| ------------------------------------- | ------------ |
| A directory Whisker cannot read       | Ends the run |
| An ignore file Whisker cannot parse   | Ends the run |
| A file Whisker cannot read or analyze | Ends the run |
| A run that finds nothing to check     | Fails        |

The first two change which files Whisker inspects, so continuing would report
on a different set than the project asked for.

Pass `--keep-going` to report each failure, continue, and still exit non-zero.

## What fails a run

| Severity | Fails by default | Fails with `--deny-warnings` |
| -------- | ---------------- | ---------------------------- |
| `Error`  | Yes              | Yes                          |
| `Warn`   | No               | Yes                          |
| `Info`   | No               | No                           |
| `Help`   | No               | No                           |

`--deny-warnings` also renders a promoted warning as an error, so the output
matches the exit status. Nothing promotes `Info` or `Help`.

## Files without coverage

A file that no provider covers is an error, and the run fails. Whisker prints
one `help:` line per distinct reason after the per-file errors:

| Reason                                 | `help:` line                                                                             |
| -------------------------------------- | ---------------------------------------------------------------------------------------- |
| Outside the loaded root                | check the file from inside its own project, or exclude it from whisker                   |
| Nothing loaded reaches it              | reference the file from a source the toolchain already loads, or exclude it from whisker |
| The toolchain excludes it              | remove the file from the toolchain's exclusion list, or exclude it from whisker          |
| Text differs from the toolchain's copy | re-run whisker                                                                           |

[Why a run refuses what it cannot see][coverage] explains the design.

## Where output goes

Diagnostics, errors, and `help:` lines all render on stderr. See
[command line][command-line] for exit status.

[command-line]: /docs/reference/command-line
[coverage]: /docs/explanation/coverage
