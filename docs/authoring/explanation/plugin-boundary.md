# The plugin boundary

Rust has no stable ABI. A loaded library agrees with the Whisker binary only
when the same rustc compiled both and both lay the boundary out the same way.
The loader establishes that before it calls anything the plugin defines.

## The handshake

A plugin exports a declaration. The loader reads its leading protocol version
through a raw pointer, and only a matching version licenses a reference to the
whole struct. It then compares, in order:

1. The protocol version.
2. The rustc version string.
3. A fingerprint of whisker-types.
4. A fingerprint of whisker-rust.

The first mismatch refuses the library, with an error naming what to rebuild.
Only then does the loader call the plugin's registration function.

## What it is guarding against

Silence. A plugin built against drifted source would mostly work, and rules
[fail open][how-it-works], so a wrong answer would pass unnoticed. The
handshake turns a subtle wrong answer into a loud refusal.

That is also why there is no flag to skip it. A mismatched layout is undefined
behavior rather than a wrong result.

## What the fingerprints cover

Each fingerprint names a list of types that cross the boundary and records the
size and alignment of every one. For `Diagnostic`, `Span`, `Suggestion`,
`Location`, and `DecoratedNode` it also records every field offset. The
whisker-rust fingerprint hashes the generated lint pass trait as text, because
a trait has no layout a const can read.

Doc comments and private helpers move nothing, so a plugin survives most of
Whisker's own churn.

Method order is invisible to layout, because a vtable orders its methods by
declaration. That belongs to the protocol version instead, and a test fails
when either trait's method list moves.

## What it does not cover

The fingerprints stop at Whisker's own layout. A fieldless enum keeps its size
when its variants reorder. A decoration's key hashes its definition text, so an
edit there changes the key without changing a fingerprint. The plugin's
lockfile resolves its own tree-sitter, and each image carries its own copy of
the C library.

Nothing on the call path catches a panic, and nothing checks the plugin's
allocator. A plugin must not set a `#[global_allocator]`, because the host
frees values the plugin allocated.

The practical consequence is [matching the toolchain][toolchain]. The trust
question is separate; see [pinning and trust][pinning-and-trust].

[how-it-works]: /docs/explanation/how-whisker-works#rules-fail-open
[pinning-and-trust]: /docs/explanation/pinning-and-trust
[toolchain]: /authoring/how-to/match-the-toolchain
