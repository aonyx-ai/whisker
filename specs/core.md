# whisker-core

The engine. Drives the pipeline and walks the tree. Depends on `whisker-types`
for all domain types.

## Tree walker

r[core.tree-walker]
The tree walker drives traversal of the decorated syntax tree. It calls lint
passes for each node and collects diagnostics.

## Pipeline

r[core.pipeline.parse]
The pipeline must parse source files using tree-sitter with the grammar
appropriate for the file's language.

r[core.pipeline.decorate]
The pipeline must invoke decoration providers to attach semantic information to
the parsed syntax tree before executing lint passes.

r[core.pipeline.execute]
The pipeline must walk the decorated tree and invoke each enabled lint pass via
the tree walker, collecting all emitted diagnostics.

r[core.pipeline.language-detection]
The pipeline must detect the language of each source file and select the
appropriate language SDK for parsing and decoration.
