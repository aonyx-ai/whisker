# Install Whisker

## With the install script

The script picks the archive for the machine it runs on, checks it against the
digest published beside it, and puts the binary in `~/.local/bin`:

```bash
curl -LsSf https://aonyx-ai.github.io/whisker/install.sh | sh
```

Set `WHISKER_INSTALL_DIR` to install somewhere else. The script installs the
newest release over whatever Whisker that directory already holds.

## By hand

Download the archive for your platform from the [releases page][releases],
check it against the `.sha256` beside it, and unpack it:

```bash
shasum -a 256 -c whisker-0.1.0-rc.4-aarch64-apple-darwin.tar.gz.sha256
tar -xzf whisker-0.1.0-rc.4-aarch64-apple-darwin.tar.gz
```

The archive unpacks to a directory named after the release and the platform. It
holds the binary, both licenses, and the README. Move `whisker` to a directory
on your `PATH`, such as `~/.local/bin`.

## From source

Whisker also builds from source. `rust-toolchain.toml` pins a nightly
toolchain, and rustup installs it during the build:

```bash
cargo install --git https://github.com/aonyx-ai/whisker --locked whisker
```

A build from source is the simpler choice for a project that compiles its own
rules, because you then hold the toolchain that built the binary. See
[matching the toolchain][toolchain].

## Next

- [Supported platforms][platforms]: the archives, the glibc floor, and the
  toolchain each release carries.
- [Run Whisker on GitHub Actions][actions]: the same install on a runner.
- [Configuration][configuration]: point Whisker at a set of rules.

[actions]: /docs/how-to/github-actions
[configuration]: /docs/reference/configuration
[platforms]: /docs/reference/platforms
[releases]: https://github.com/aonyx-ai/whisker/releases
[toolchain]: /authoring/how-to/match-the-toolchain
