# Package and ABI versioning

<!--
goal: make a consumer see that their rules pin and their whisker version are
one thing that moves together, before a build breaks and tells them.
non-goal: how the layout check works. the plugin boundary page in the
authoring tree owns the mechanism, and this one never repeats it.
non-goal: whether a pinned repository is safe to run. pinning-and-trust owns
that, and a version match is not a trust decision.

what binds them: why is my rules pin tied to my whisker at all. a rules
repository ships plugins, and a plugin only loads into a whisker that lays the
boundary types out the same way.
what is compared: what actually has to match. a protocol version and two
fingerprints of whisker's own source. the compiler is not one of them, so any
rustc can have built either side.
what it costs: how often do I pay this. every whisker release before 1.0,
because major 0 promises nothing and a plugin loads only on the whisker it was
built for. reaching 1.0 turns that into a promise the major carries.
the order: which do I move first. the whisker, then the rules pin to a commit
built against it. the reverse leaves you pinned to rules nothing can load.
how do I tell: what do I run. `whisker abi` prints the tag this whisker loads,
and a mismatch names both sides on stderr. troubleshooting carries the exact
messages.
what a bot cannot do: why did renovate not handle this. it watches the rules
repository's releases, so it cannot know your whisker moved, and it cannot pin
a commit nobody has pushed yet.
-->
