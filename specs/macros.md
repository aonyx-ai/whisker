# whisker-macros

Generator infrastructure. Provides the `#[derive(Decoration)]` proc macro and
the visitor codegen tooling that reads tree-sitter's `node-types.json`.

The two generators are independent. They consume different inputs, have
different change cadences, and can be versioned separately.

## Visitor generator

r[macros.visitor.input]
The visitor generator takes tree-sitter's `node-types.json` for a language
grammar as input.

r[macros.visitor.output]
The visitor generator produces a `{Language}LintPass` trait with one
`check_{node_kind}` method per named node type in the grammar.

r[macros.visitor.node-kind]
Each named node type from the grammar (e.g. `function_item`,
`let_declaration`) becomes a method on the generated visitor trait.

r[macros.visitor.supertype]
Abstract groupings defined in the grammar (e.g. `_expression` grouping
`call_expression`, `binary_expression`, etc.) must generate a method that fires
for all subtypes. The supertype method fires in addition to the concrete node
method, not instead of it.

r[macros.visitor.default-impl]
All methods on the generated visitor trait must default to returning an empty
diagnostic list, so rule authors only implement the methods they care about.

r[macros.visitor.dispatch]
The generated code must include dispatch glue that bridges from the platform's
`LintPass::check_node` to the appropriate typed method based on the node's
kind.

## Decoration generator

r[macros.decoration.input]
The decoration generator takes Rust types annotated with
`#[derive(Decoration)]` as input.

r[macros.decoration.output]
The decoration generator produces a `{Language}Decorations` trait with accessor
methods, implemented on `DecoratedNode`.

r[macros.decoration.cardinality]
The `Decoration` derive must support declaring cardinality (`one` or `many`) on
the authored type. Cardinality determines the accessor return type:
`Option<&T>` for `one`, `&[T]` for `many`.

r[macros.decoration.payload]
Decoration payloads may be enums, structs, or recursive types, as defined by
the authored Rust type.

r[macros.decoration.independence]
A tree-sitter grammar update must be able to change the visitor trait without
affecting the decoration trait, and vice versa.
