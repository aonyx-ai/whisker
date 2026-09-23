# Plugin boundary

<!--
goal: make a rule author understand why whisker refuses a plugin, and which
part of that they control.
non-goal: the mechanism. how a fingerprint is derived, and which change moves
which component, is whisker's own business and nobody here acts on it.
non-goal: the trust question, which pinning-and-trust owns, and the republish
schedule, which the prebuilt archives reference owns.

no stable ABI: why does this check exist at all. rust promises no layout
across compilers, so whisker and a plugin have to be shown to agree before
anything is called.
what decides agreement: what is actually compared. the whisker source each
side was built from. three values: a version and two fingerprints.
the compiler is absent: must I match a toolchain. no. build a rule with
whatever rustc you like. this used to be required and is worth saying plainly,
because the old answer is still in people's heads.
guarding against silence: why refuse instead of carrying on. rules fail open,
so a plugin built against drifted source would go quiet rather than wrong, and
nobody would see it.
no skip flag: can I override it. no. a mismatched layout is undefined
behavior, not a wrong answer.
what I control: what is mine to get right. one thing. pin the whisker crates
to the whisker that will load the plugin.
how often that costs me: when do I rebuild. before 1.0, every whisker release.
from 1.0, only when the major moves.
what crosses safely: what am I allowed to do inside a rule. my own global
allocator, and a panic that comes back to whisker as a value rather than
ending the process.
what stays my problem: what the handshake does not check. the rest of my
lockfile. it is outside the boundary, so commit it and keep it in step.
-->
