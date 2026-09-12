# Supported platforms

Every release carries an archive per platform, each with a `.sha256` beside it.

| Platform               | Target                      |
| ---------------------- | --------------------------- |
| Linux on x86-64        | `x86_64-unknown-linux-gnu`  |
| Linux on arm64         | `aarch64-unknown-linux-gnu` |
| macOS on Apple silicon | `aarch64-apple-darwin`      |

The Linux binaries need glibc 2.35 or newer, which Ubuntu 22.04 and Debian 12
satisfy. Build [from source][installation] on anything older, and on any
platform not listed.

[installation]: /docs/how-to/install
