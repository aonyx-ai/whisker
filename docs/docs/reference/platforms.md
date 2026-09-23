# Platforms

<!--
goal: tell a reader whether their machine has a prebuilt archive.
non-goal: install instructions. install.md owns those.
-->

## the table

<!--
is my platform here. three targets, each with a .sha256 beside it.
will the Linux binary run on my distro. 2.35, which Ubuntu 22.04 and Debian 12
satisfy.
what if I am not listed. build from source.
-->

| Platform               | Target                      |
| ---------------------- | --------------------------- |
| Linux on x86-64        | `x86_64-unknown-linux-gnu`  |
| Linux on arm64         | `aarch64-unknown-linux-gnu` |
| macOS on Apple silicon | `aarch64-apple-darwin`      |

<!--
this page is one table and two sentences. it may belong inside install.md rather
than standing alone.
-->
