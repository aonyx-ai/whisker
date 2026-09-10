# Pinning and trust

## Why a commit, never a branch

A branch or a tag names whatever the remote points it at today. The same
configuration would then check the same code against different rules on
different days, and a linter that changes underneath a project is not a linter
anyone can act on.

An abbreviated hash grows ambiguous as a repository gains objects, so Whisker
takes the full 40 characters and nothing shorter.

A pin is also a cache key. A commit hash names an immutable tree, so Whisker
never refreshes a checkout that exists, and never asks the remote about it
again.

## What the digest proves

That a downloaded archive arrived intact. Nothing more.

The same publisher writes the archive and the digest, so a matching digest says
nothing about whether the contents deserve to run.

## What the handshake proves

Compatibility. That a library was built by the same compiler and against the
same Whisker source, so the two agree on memory layout. See
[the plugin boundary][plugin-boundary].

Neither check is a trust decision, and neither was meant to be.

## Where trust actually comes from

The configuration. A repository named in `.config/whisker.toml` runs its code
inside Whisker's process, whether it arrived prebuilt or compiled from source.
A lint crate is not sandboxed, and `dlopen` runs a library's initializers
before the handshake reads anything.

Pin rules you would be willing to run as a build script, and read the diff when
you move a pin.

[plugin-boundary]: /authoring/explanation/plugin-boundary
