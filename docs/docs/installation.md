---
sidebar_position: 1
---

# Installation

Every release carries an archive for Linux on x86-64 and arm64, and for macOS
on Apple silicon. The Linux binaries need glibc 2.35 or newer, which Ubuntu
22.04 and Debian 12 satisfy. Build from source on anything older.

## With the install script

The script picks the archive for the machine it runs on, checks it against the
digest published beside it, and puts the binary in `~/.local/bin`:

```bash
curl -LsSf https://aonyx-ai.github.io/whisker/install.sh | sh
```

Set `WHISKER_INSTALL_DIR` to install somewhere else. The script installs the
newest release over whatever whisker that directory already holds.

## By hand

Download the archive for your platform from the [releases page][releases],
check it against the `.sha256` beside it, and unpack it:

```bash
shasum -a 256 -c whisker-0.1.0-rc.3-aarch64-apple-darwin.tar.gz.sha256
tar -xzf whisker-0.1.0-rc.3-aarch64-apple-darwin.tar.gz
```

The archive unpacks to a directory named after the release and the platform. It
holds the binary, both licenses, and the README. Move `whisker` to a directory
on your `PATH`, such as `~/.local/bin`.

## On GitHub Actions

The action in the whisker repository does the same download, check, and unpack,
for the runner it finds itself on:

```yaml
- uses: aonyx-ai/whisker@v0.1.0-rc.3
  with:
    version: v0.1.0-rc.3
- run: whisker check .
```

## From source

Whisker also builds from source. `rust-toolchain.toml` pins a nightly
toolchain, and rustup installs it during the build:

```bash
cargo install --git https://github.com/aonyx-ai/whisker --locked whisker
```

## Which one to choose

Both load the same custom lints. A plugin is laid out by stabby, so whisker
loads it regardless of which toolchain built it, and a released binary and a
build from source differ only in how you obtained whisker. What a plugin and
its whisker must agree on is the fingerprints of the types that cross between
them, and pinning the plugin to the revision your whisker was built from is the
sure way to match them. [Custom lints](/docs/custom-lints) covers the handshake
this rests on.

[releases]: https://github.com/aonyx-ai/whisker/releases
