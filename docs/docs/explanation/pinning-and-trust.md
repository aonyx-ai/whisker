# Pinning and trust

<!--
goal: separate three things a reader conflates, a pin, a digest and the
handshake, and land on where trust actually lives.
non-goal: how to pin. shared-rules covers the mechanics.
why a commit: why not a branch. a linter that changes underneath a project is
not one anyone can act on.
why the full hash: why not an abbreviation. it grows ambiguous as a repository
gains objects.
a pin is a cache key: what else does the commit buy. an immutable tree, so no
refresh and no second question to the remote.
what the digest proves: does a matching digest mean safe. no. the same publisher
writes the archive and the digest.
what the handshake proves: does loading successfully mean safe. no. only that
both sides were built against the same whisker source, and the compiler is not
part of it.
neither is trust: what have we actually established. two compatibility checks
and no trust decision, said outright.
where trust comes from: what decides it then. the configuration. a named
repository runs unsandboxed in-process, and dlopen runs initializers before the
handshake reads anything.
the instruction: what do I do with that. pin what you would run as a build
script, and read the diff when you move a pin.
-->
