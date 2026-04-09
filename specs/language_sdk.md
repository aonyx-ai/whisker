# whisker-language

Each language SDK combines generated and authored code into a single crate that
rule authors depend on. Language SDKs depend on `whisker-types` for the core
domain types (`DecoratedNode`, `Diagnostic`, `LintPass`, etc.).

## Generated visitor trait

r[sdk.visitor.trait]
The language SDK must expose a `{Language}LintPass` trait generated from the
language's `node-types.json` by the visitor generator.

r[sdk.visitor.per-language]
Node kinds are completely language-specific. Each language SDK must generate its
own visitor trait from its own grammar. There is no universal taxonomy.

## Generated decoration trait

r[sdk.decorations.trait]
The language SDK must expose a `{Language}Decorations` trait generated from the
authored decoration types by the decoration generator.

r[sdk.decorations.on-decorated-node]
The generated decoration trait must be implemented on `DecoratedNode`, so rule
authors can call accessors directly on nodes after bringing the trait into
scope.

## Authored provider implementation

r[sdk.provider.toolchain-connection]
The provider must manage the connection to the language toolchain (LSP, direct
library linking, CLI invocation, or other mechanism).

r[sdk.provider.translation]
The provider must translate toolchain-internal types into the SDK's decoration
types.

r[sdk.provider.scope]
The provider must handle the toolchain's analysis scope (file, crate,
workspace) and its implications for when decorations are available.
