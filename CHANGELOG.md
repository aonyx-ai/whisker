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

- Whisker's own rules moved to
  [whisker-aonyx-rules](https://github.com/aonyx-ai/whisker-aonyx-rules). A
  project that ran them from `lints/` now names that repository and a commit.
