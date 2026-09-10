# How Whisker works

Most of what a lint needs to know is syntactic, and syntax is cheap. A
tree-sitter grammar parses a file in isolation, with no toolchain, no build,
and no valid project.

Some lints need facts a syntax tree cannot supply. Is this match scrutinee an
enum? Does this function return `Result`? Semantics is expensive, because it
needs a real toolchain with knowledge of the whole project.

Whisker keeps the two apart. It parses files with tree-sitter and walks the
tree through lint rules. Semantic facts enter as **decorations**: typed values
that a language provider computes up front and attaches to individual nodes.
Rules read decorations off the tree and never talk to a toolchain.

## The pipeline

1. **Parse.** One parser, fixed to the Rust grammar, built before any file is
   visited. The tree, its text, and an empty decoration map form a decorated
   tree.
2. **Decorate.** Every provider is offered the tree, and either covers the file
   or declines it with a reason. This is the only stage that touches a
   toolchain.
3. **Execute.** A depth-first walk offers every named node to every lint pass
   and collects what they return.
4. **Report.** After the last file, every diagnostic renders to stderr with its
   span, annotations, and suggested fixes.

The pipeline is synchronous and handles one file at a time. Passes hold state,
so Whisker builds a fresh set for each file.

## Why the split pays

A rule is a function of a decorated tree. A test can hand it decorations it
built itself, and the rule still makes type-aware decisions without a project
or a toolchain anywhere near it. That is what makes rules cheap to write and
cheap to test.

## Rules fail open

A decoration is evidence, never a requirement. Where a decoration is the reason
to report, a missing one keeps the rule silent. Where a decoration would exempt
the code, a missing one leaves the diagnostic standing.

Whisker prefers a missed finding to a finding it cannot justify. A rule that
guessed from absent evidence would be wrong in a way nobody could see.

This is also why a file with no coverage is an error rather than a file checked
by syntactic rules alone. See
[why a run refuses what it cannot see][coverage].

[coverage]: /docs/explanation/coverage
