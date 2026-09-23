# Publish prebuilt archives

<!--
goal: let a publisher of rules ship them compiled, so no consumer pays the build.
non-goal: what a consumer sees when an archive is missing or broken. that is
the reference page of the same name in the usage tree.
-->

## the archive

<!--
what must I name the file. the four rows of the table.
what is `<tag>` made of. the two fingerprints, the protocol, and the triple. no
compiler, so one archive serves every whisker built from the same boundary.
whose tag is it. the consumer's, not yours. `whisker abi` prints it, so publish
one archive per tag you mean to serve.
what may the archive hold. regular files at the root only. whisker passes over
anything else, so a library in a subdirectory is a library nobody loads.
-->

| Part     | Value                                               |
| -------- | --------------------------------------------------- |
| Name     | `<rev>-<tag>.tar.gz`                                |
| `<rev>`  | The commit a project pins                           |
| `<tag>`  | What `whisker abi` prints on the consumer's machine |
| Digest   | A `.sha256` published beside the archive            |
| Contents | Regular files at the archive root, and nothing else |

## commit the lockfile

<!--
what else do I owe consumers. a committed lockfile, because a git entry builds
with --locked. without one every consumer's build fails, archives or not.
-->

## when the tag moves

<!--
when do my archives stop being found. when the boundary moves, because the tag
carries the fingerprints and the protocol floor. a whisker release that changes
either one wants a fresh archive.
why does a bad archive fail loudly. a library that fails the handshake carries a
tag that misdescribes it, so whisker ends the run rather than quietly compiling.
-->
