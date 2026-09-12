# The plugin boundary

Rust has no stable ABI of its own. Every type that crosses between Whisker and
a plugin is therefore laid out by [stabby][stabby], whose layouts hold under
any compiler. A loaded library agrees with the Whisker binary when both lay the
boundary out the same way. That is a property of the Whisker source each was
built from, and not of the rustc that built it. The loader establishes it
before it calls anything the plugin defines.

## The handshake

A plugin exports a declaration. The loader reads its leading protocol version
through a raw pointer, and only a version it reads licenses a reference to the
whole struct. It then compares, in order:

1. The protocol version.
2. A fingerprint of whisker-types.
3. A fingerprint of whisker-rust.

The first mismatch refuses the library, with an error naming what to rebuild.
Only then does the loader ask the plugin for its rules and its lint passes.

The compiler is not on that list. Whisker loads a plugin whatever rustc built
it, so the toolchain you build a rule with is yours to choose.

## What it is guarding against

Silence. A plugin built against drifted source would mostly work, and rules
[fail open][how-it-works], so a wrong answer would pass unnoticed. The
handshake turns a subtle wrong answer into a loud refusal.

That is also why there is no flag to skip it. A mismatched layout is undefined
behavior rather than a wrong result.

## What the fingerprints cover

Each fingerprint names a list of types that cross the boundary. Every one of
them is laid out by stabby, and each contributes the identity stabby derives
from its report. That identity covers the type's name, its module, and the name
and type of every field, recursively. A field that moved, or that changed to
another type of the same size, is refused.

The whisker-rust fingerprint also hashes the generated lint pass trait as text,
because a trait has no layout a const can read.

Doc comments and private helpers move nothing, so a plugin survives most of
Whisker's own churn.

## What the version covers

The protocol version is a major and a minor. It guards what no fingerprint can
read back: the shape of the declaration, the meaning of its fields, and the
signatures of the pass trait's methods.

A field appended to the declaration raises the minor. The declaration is
`#[repr(C)]`, so Whisker knows where each version's fields end, and a plugin
built before that field simply offers less. A method added to the pass trait
raises the major instead, because a vtable orders its methods by declaration
and no offset arithmetic reaches around that.

Whisker is before 1.0, where nothing is promised. A plugin loads only on the
Whisker it was built for, and every change to the boundary costs one minor.
From 1.0 the major carries the promise, and a plugin built for one minor of a
major keeps loading on every later minor of it.

## What it does not cover

The fingerprints stop at Whisker's own layout. A fieldless enum keeps its size
when its variants reorder. A decoration's key hashes its definition text, so an
edit there changes the key without changing a fingerprint. The plugin's
lockfile resolves its own tree-sitter, and each image carries its own copy of
the C library.

The rest of the graph your lockfile resolves is outside the handshake too.
Commit that lockfile and keep it in step with the Whisker you build against.

## What crosses safely

Every allocation that crosses carries the function that frees it, so the side
that allocated a value is the side that frees it. A plugin may therefore set
its own `#[global_allocator]`.

A panic inside a rule comes back to Whisker as a value rather than unwinding
across the boundary. Whisker names the file and the node the rule was checking,
and the run ends there.

A node reaches Whisker's decorations through a call rather than a reference to
a map, so no standard-library collection crosses.

The practical consequence for a rule author is pinning the Whisker
dependencies; see [write a rule][write-a-rule]. The trust question is separate;
see [pinning and trust][pinning-and-trust].

[how-it-works]: /docs/explanation/how-whisker-works#rules-fail-open
[pinning-and-trust]: /docs/explanation/pinning-and-trust
[stabby]: https://crates.io/crates/stabby
[write-a-rule]: /authoring/how-to/write-a-rule
