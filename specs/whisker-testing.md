# whisker-testing

## Test harness

r[testing.parse]
The test harness must parse source text into a tree-sitter syntax tree for the
specified language.

r[testing.decorate]
The test harness must accept manually constructed decorations and attach them to
parsed syntax trees, so rule authors can test against specific semantic states
without a running language toolchain.

r[testing.execute]
The test harness must execute a lint pass against a decorated tree and return
the emitted diagnostics.

r[testing.assert-diagnostic]
The test harness must provide assertions for verifying diagnostic properties:
rule ID, message, primary span, origins, related locations, and suggestions.

r[testing.assert-no-diagnostic]
The test harness must provide assertions for verifying that a lint pass emits no
diagnostics for a given input.

## Fixture loading

r[testing.fixture.directory]
The test harness must load test fixtures from a directory, where each file is a
separate test case.
