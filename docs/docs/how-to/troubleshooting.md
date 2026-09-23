# Troubleshooting

<!--
goal: let a reader who has an error on stderr find the cause and the fix by
searching for the words they are looking at.
non-goal: explaining the designs behind the refusals. each entry links to the
explanation that owns its reasoning.
entries are keyed by the message, because that is what the reader has. the
messages live in the code, so this page goes stale in a way the explanation
pages do not. pin them with a test if this grows.
-->

When you run into an error in Whisker, check out this page to find out how to
resolve it.

## plugin (ABI x.y) incompatible with whisker (ABI x.y)

Whisker rules are built against a specific version of the Whisker ABI.
Sometimes, we make backwards-incompatible changes to the Whisker ABI. When the
plugin's ABI version is below Whisker's ABI version, it's trying to use Whisker
features from a newer Whisker version.

To resolve this, you have two options:

1. Upgrade your Whisker installation to a version that supports the required ABI
   version for your plugin; or
2. Downgrade your plugin to a version that was built against an ABI version that
   your installed Whisker supports.

:::note

While Whisker is still in pre-1.0 development, the ABI version may change in
backwards-incompatible ways without increment the major version. This means ABI
version `0.2` may be incompatible with ABI version `0.1`.

:::

## the directory holds no dynamic library; delete it and run whisker again

Whisker has a cache for remote rules. Sometimes this cache breaks in unexpected
ways, for example when interrupting a Whisker run during build.

To resolve this problem, simply delete the listed directory and rerun Whisker.

## whisker cannot check FILE: there is no grammar for `.EXT` files

You've tried to make Whisker lint a file that it does not (yet) support. To do
this, you've most likely run `whisker check <filename.ext>`.

If you ran some other command, please file an issue on our Github.

## whisker analyzed no files under PATH

Whisker's file discovery found no files to lint. This usually means you've
ignored all files, or ran Whisker before writing code in a language it supports.

Double check the [ignore patterns][ignore] in `.config/whisker.toml`.

## a lint pass panicked while checking a NODE at byte N of FILE

A rule threw an error when checking a specific file.

Report this issue to the rule author, where possible together with the code that
caused the issue. You can try downgrading to a version of the rule that works,
or temporarily disable the rule altogether.

## cannot update the lock file ... because --locked was passed

You've configured a lint using a Git source without prebuilt archives. The
repository has no `Cargo.lock` committed, or the locked dependencies no longer
resolve.

File an issue with the owner of the rules. You can try to change the hash of the
lint to a version that did commit a `Cargo.lock`.

## failed to load the target project for analysis

<!--
what happened. whisker pointed rust-analyzer at the path you named and it found
no cargo project there. the cause line reads "discover a Cargo project at".
what do I do. run whisker from inside the project, or name the project
directory on the command line.
why does whisker need one. rust-analyzer loads the nearest workspace and runs
its build scripts before it can answer anything about a type.
-->

## no decoration provider covers this file, so semantic rules cannot run

<!--
what happened. the file passed discovery, but no provider could analyze it, so
whisker refuses rather than report it clean.
which reason is mine. the error names the provider and the gap, and a help line
names the fix. whisker prints each distinct fix once, after the per-file errors.
what if it says no decoration providers were configured. nothing analyzable is
loaded at all, which is the same refusal with no provider to name.
-->

| Reason                                 | `help:` line                                                                             |
| -------------------------------------- | ---------------------------------------------------------------------------------------- |
| Outside the loaded root                | check the file from inside its own project, or exclude it from whisker                   |
| Nothing loaded reaches it              | reference the file from a source the toolchain already loads, or exclude it from whisker |
| The toolchain excludes it              | remove the file from the toolchain's exclusion list, or exclude it from whisker          |
| Text differs from the toolchain's copy | re-run whisker                                                                           |

[ignore]: /docs/reference/configuration#ignore
