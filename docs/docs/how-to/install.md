# Install Whisker

There's various ways to install Whisker. On this page we'll go through a few
options.

## Recommended: Install script

This is easily the fastest way to get started. Feel free to read through [the
installation script][install-sh] to make sure it's safe to run.

```bash
curl -LsSf https://aonyx-ai.github.io/whisker/install.sh | sh
```

If you don't want to install in `$HOME/.local/bin` you can set a different path
in the `WHISKER_INSTALL_DIR` environment variable and the script will pick that
up.

## Github Releases

Alternatively, grab the `.tar.gz` and the shasum directly from [Github
Releases][gh-releases]. You'll have to move the file someplace on your `$PATH`
yourself.

```bash
shasum -a 256 -c whisker-0.1.0-rc.4-aarch64-apple-darwin.tar.gz.sha256
tar -xzf whisker-0.1.0-rc.4-aarch64-apple-darwin.tar.gz
```

## From source

<!--
what if my platform has no archive. cargo install --git --locked, and
rust-toolchain.toml pulling the nightly.
-->

If your platform has no archive, you want to install an unreleased version, or
you don't want to install binary files from the internet, you can easily build
Whisker from source. The easiest method is to use `cargo install` pointed at the
Git repository:

```bash
cargo install --git https://github.com/aonyx-ai/whisker --locked whisker
```

Alternatively, you can clone the repository and install it from there. Note that
our workspace includes a nightly toolchain for some checks, but you can use
Cargo's `+stable` option to bypass installing it.

```bash
git clone https://github.com/aonyx-ai/whisker
cd whisker
cargo +stable install --locked --path crates/whisker
```

[install-sh]: https://github.com/aonyx-ai/whisker/blob/main/docs/static/install.sh
[gh-releases]: https://github.com/aonyx-ai/whisker/releases
