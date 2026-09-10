# Why a run refuses what it cannot see

A file that no decoration provider covers is an error. Whisker runs no rule on
it, syntactic rules included, and the run fails.

That looks strict. The alternative is worse: a file checked by syntactic rules
alone comes back clean, and a clean result is indistinguishable from a file
that every rule inspected and approved. The gap would be invisible exactly
where it matters.

Rules already [fail open][how-it-works], so a rule that needed a missing
decoration would stay silent rather than guess. Running the syntactic half
would therefore report a subset of the truth under a label that claims all of
it.

## The four reasons

A provider declines a file because it sits outside the root the provider
loaded, because nothing the toolchain loaded reaches it, because the toolchain
excluded it, or because its text differs from the toolchain's copy.

Each reason has a different fix, so Whisker prints one `help:` line per
distinct reason across the run rather than one per file. The lines are listed
in [runs and outcomes][checking-a-project].

## An empty run is an error too

A run that finds nothing to check fails. It usually means a pattern matched too
much, and it would otherwise look exactly like a clean project.

## A file with no grammar

Whisker refuses a named file it has no grammar for, rather than parsing it with
the wrong one. That parse would find nothing and report the file clean.

The theme is the same each time: Whisker would rather refuse than report a
silence it cannot account for.

[checking-a-project]: /docs/reference/runs-and-outcomes
[how-it-works]: /docs/explanation/how-whisker-works#rules-fail-open
