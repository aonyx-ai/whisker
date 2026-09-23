# How Whisker works

<!--
goal: give a reader the one idea that makes the rest predictable, the syntax and
semantics split.
non-goal: API detail. write-a-rule and api own that.
-->

## Syntax is cheap

<!--
syntax is cheap: why tree-sitter. it parses in isolation, with no toolchain, no
build and no valid project.
-->

## Some lints need more

<!--
some lints need more: what can syntax not answer. is this scrutinee an enum,
does this function return Result.
-->

## Decorations

<!--
decorations: how do the two halves meet. typed values a provider computes up
front and attaches to nodes, so rules never talk to a toolchain.
-->

## The pipeline

<!--
the pipeline: what happens in what order. parse, decorate, execute, report, and
decorate is the only stage touching a toolchain.
-->

## One file at a time

<!--
one file at a time: why a fresh pass per file. passes hold state.
-->

## Why the split pays

<!--
why the split pays: what do I get for it. a rule is a function of a decorated
tree, so a test hands it decorations and needs no project.
-->

## Rules fail open

<!--
rules fail open: what happens when evidence is missing. silent where a
decoration is the reason to report, standing where it would have exempted.
-->

## The preference

<!--
the preference: why prefer a miss to a guess. a finding whisker cannot justify
is wrong in a way nobody can see.
-->

## The handoff

<!--
the handoff: so why is an uncovered file an error. running the syntactic half
alone would return clean, and clean is indistinguishable from fully inspected.
troubleshooting has the four reasons and their remedies.
-->
