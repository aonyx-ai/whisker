<!-- markdownlint-disable-file MD024 -->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Whisker asks a git source's releases for prebuilt lints before it compiles
  anything. It verifies the published SHA-256, and every library still
  completes the plugin handshake.
- An action installs a released whisker on a GitHub Actions runner. It picks
  the archive for the runner, checks it against the digest published beside
  it, and puts whisker on the `PATH`.
- `whisker abi` prints the tag that says which prebuilt lints this binary
  loads. Publishers put it in each archive's name.
- Every release carries whisker binaries for Linux on x86-64 and arm64 and for
  macOS on Apple silicon, each with a SHA-256 beside it. Prereleases too.
- A `[[lints]]` entry can name a repository and a commit, which whisker caches.
  A directory may also hold a cargo workspace.

### Changed

- The Linux binaries are built on Ubuntu 22.04, so they need glibc 2.35 rather
  than 2.39. The 2.39 floor refused Ubuntu 22.04 LTS and Debian 12.
- Whisker's own rules moved to [whisker-aonyx-rules][rules]. A project that ran
  them from `lints/` now names that repository and a commit.
- Whisker finds its project with [kawauso-project][kawauso], and a broken
  configuration file reports the line and column to fix.
- Whisker lists its subcommands in a stable order.

### Removed

- Whisker no longer reads `.whisker.toml`. Move it to `.config/whisker.toml`.
  Whisker says nothing about the old name, so a project that misses this runs
  with no patterns and no custom lints.
- The crates.io publish workflow is gone. Releases carry binaries.

[kawauso]: https://crates.io/crates/kawauso-project
[rules]: https://github.com/aonyx-ai/whisker-aonyx-rules
