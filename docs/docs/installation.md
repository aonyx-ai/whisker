---
sidebar_position: 1
---

# Installation

Every release carries an archive for Linux on x86-64 and arm64, and for macOS
on Apple silicon. Download the one for your platform from the
[releases page][releases], check it against the `.sha256` beside it, and unpack
it:

```bash
shasum -a 256 -c whisker-0.1.0-rc.3-aarch64-apple-darwin.tar.gz.sha256
tar -xzf whisker-0.1.0-rc.3-aarch64-apple-darwin.tar.gz
```

The archive unpacks to a directory named after the release and the platform. It
holds the binary, both licenses, and the README. Move `whisker` to a directory
on your `PATH`, such as `~/.local/bin`.

The Linux binaries need glibc 2.35 or newer, which Ubuntu 22.04 and Debian 12
satisfy. Build from source on anything older.

## On GitHub Actions

The action in the whisker repository does the same three steps, for the runner
it finds itself on:

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

The choice decides how whisker obtains your custom lints. A released binary
loads only a library built by the nightly that built it, so a project that
compiles its own rules needs that same toolchain. Building whisker from source
lets you compile both with whatever `rust-toolchain.toml` names at that commit.
[Custom lints](/docs/custom-lints) covers the handshake this rests on.

[releases]: https://github.com/aonyx-ai/whisker/releases
