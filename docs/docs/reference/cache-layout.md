# Cache layout

Whisker caches what it fetches and builds for a `[[lints]]` git entry. The root
is `WHISKER_CACHE_DIR`, or the XDG cache directory plus `whisker`, which is
`~/.cache/whisker` on every platform.

| Path                                     | Holds                                                                |
| ---------------------------------------- | -------------------------------------------------------------------- |
| `<cache>/git/<remote>/<rev>/`            | A checkout of the pinned commit, and cargo's target directory for it |
| `<cache>/prebuilt/<remote>/<rev>/<tag>/` | Libraries unpacked from a published archive                          |

`<remote>` is a readable slug of the repository plus a digest of the whole
remote, because two remotes can share a last path segment. `<tag>` is what
`whisker abi` prints.

Whisker assembles each directory beside its destination and renames it into
place, so a directory that exists is whole.

## Clearing it

| To force                            | Delete                             |
| ----------------------------------- | ---------------------------------- |
| A fresh look for published archives | `<cache>/prebuilt/<remote>/<rev>/` |
| A fresh fetch and compile           | `<cache>/git/<remote>/<rev>/`      |

Whisker prefers unpacked libraries, then an existing checkout, and only then
asks the remote. A project with a warm checkout therefore keeps compiling after
its rules start publishing archives, until the pin moves or the checkout goes.

A commit hash names an immutable tree, so Whisker never refreshes a checkout
that exists.
