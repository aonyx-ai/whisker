# Cache layout

<!--
goal: let someone find, inspect, or delete what whisker cached.
non-goal: justifying the pin model. pinning-and-trust does that.
-->

## the layout

<!--
where is the root and what lives under it. WHISKER_CACHE_DIR else the XDG
directory, then the two paths.
why does the remote directory look like that. a readable name plus a digest,
because two remotes can share a last segment.
can I trust a directory I find. assembled beside the destination and renamed, so
one that exists is whole.
-->

| Path                                     | Holds                                                                |
| ---------------------------------------- | -------------------------------------------------------------------- |
| `<cache>/git/<remote>/<rev>/`            | A checkout of the pinned commit, and cargo's target directory for it |
| `<cache>/prebuilt/<remote>/<rev>/<tag>/` | Libraries unpacked from a published archive                          |

## clearing it

<!--
how do I force a fresh look. one row per intent, each naming the directory to
delete.
why does my project still compile after its rules start publishing. unpacked
libraries, then a checkout, then the remote.
why is a checkout never refreshed. a commit hash names an immutable tree.
-->

| To force                            | Delete                             |
| ----------------------------------- | ---------------------------------- |
| A fresh look for published archives | `<cache>/prebuilt/<remote>/<rev>/` |
| A fresh fetch and compile           | `<cache>/git/<remote>/<rev>/`      |
