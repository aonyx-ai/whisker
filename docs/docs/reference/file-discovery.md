# File discovery

<!--
goal: answer "why was this file skipped" and "why was that one checked".
non-goal: the ignore pattern syntax, which configuration#ignore owns.
-->

Whisker has a fairly simple algorithm to discover which files to lint. However,
sometimes it may include or skip a file unexpectedly. Refer to this page to find
out how Whisker discovers files.

## which files whisker collects

<!--
what decides collection. the extension picks the grammar, and rust support
covers .rs.
what about every other file. nothing at all. a walk passes over a file with no
grammar in silence, so a repository full of yaml produces no mention of yaml.
is being collected enough. no, and this is the part that bites. a file whisker
did collect can still turn out to be one no provider can analyze, which is an
error. a file it never collected cannot.
-->

## what the walk skips

<!--
which rules apply and where does each come from. six rows, from the walk itself
to whisker.toml.
do these apply without git. yes. an ignore file still describes what its tree
generates.
-->

| Skipped                                 | Source                 |
| --------------------------------------- | ---------------------- |
| Hidden files and directories            | The walk               |
| Anything a `.gitignore` excludes        | The repository         |
| Anything an `.ignore` excludes          | The repository         |
| Anything `.git/info/exclude` excludes   | The repository         |
| Anything your global gitignore excludes | Your git configuration |
| Anything an `ignore` pattern excludes   | `.config/whisker.toml` |

## paths you name

<!--
can I force one file. a named path is always checked, and ignores still apply
below a named directory.
why did whisker refuse my file outright. a parse with the wrong grammar finds
nothing and reports it clean.
-->
