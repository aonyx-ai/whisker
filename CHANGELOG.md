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

### Changed

- Whisker's own rules moved to [whisker-aonyx-rules][rules]. A project that
  ran them from `lints/` now names that repository and a commit.
- Whisker finds its project with [kawauso-project][kawauso], which is how
  every Aonyx tool finds one. A broken configuration file now reports the
  line and column, or the field, that has to change.

### Removed

- Whisker no longer reads `.whisker.toml`. The configuration file is
  `.config/whisker.toml`, and a project that keeps the old name has to move
  it. Whisker does not report the old file, so a project that misses this
  runs with no patterns and no custom lints.

[kawauso]: https://crates.io/crates/kawauso-project
[rules]: https://github.com/aonyx-ai/whisker-aonyx-rules
