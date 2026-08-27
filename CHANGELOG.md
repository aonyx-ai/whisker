<!-- markdownlint-disable-file MD024 -->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A `[[lints]]` entry can name a repository and a commit rather than a
  directory, and whisker fetches that commit into a cache before building it.
  A configured directory may also hold a cargo workspace, in which case every
  plugin it builds is loaded.
- Whisker looks for prebuilt lints before it compiles a git entry. It asks the
  repository's releases for an archive named after the commit and after this
  binary's tag, checks it against the digest published beside it, and loads
  what it unpacks. Every library still completes the plugin handshake. A
  repository that publishes nothing for this whisker is compiled from source
  as before, and whisker says nothing about it.
- `whisker abi` prints the tag that names which prebuilt lints this binary can
  load. Whoever publishes lints writes it into the name of each archive.
- Every release carries an archive of the whisker binary for Linux on x86-64
  and arm64, and for macOS on Apple silicon, with a SHA-256 digest beside it.

### Changed

- Whisker lists its subcommands in a stable order. The order used to come from
  however the linker laid out the command registry, so two builds could print
  two different orders.

- Whisker's own rules moved to [whisker-aonyx-rules][rules]. A project that
  ran them from `lints/` now names that repository and a commit.
- Whisker finds its project with [kawauso-project][kawauso], which is how
  every Aonyx tool finds one. A broken configuration file now reports the
  line and column, or the field, that has to change.

### Removed

- The workflow that published to crates.io is gone. It called a recipe this
  repository never had, and every crate here sets `publish = false`, so it
  could not have worked. Releases carry binaries instead.
- Whisker no longer reads `.whisker.toml`. The configuration file is
  `.config/whisker.toml`, and a project that keeps the old name has to move
  it. Whisker does not report the old file, so a project that misses this
  runs with no patterns and no custom lints.

[kawauso]: https://crates.io/crates/kawauso-project
[rules]: https://github.com/aonyx-ai/whisker-aonyx-rules
