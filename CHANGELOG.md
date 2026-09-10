<!-- markdownlint-disable-file MD024 -->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `DecoratedNode` is `Copy`. A rule reading a node out of the vector that
  `named_children` returns no longer clones borrowed data. The
  plugin ABI tag does not move, so every published archive still loads.

- Whisker says when a git source publishes no prebuilt lints it can load, and
  names the archive it looked for. The compile that follows costs minutes on
  every machine, and silence read exactly like a warm cache. A repository
  whisker cannot see, which is what a private one looks like without a token,
  stays quiet: nobody reading that can act on it.
- A `[rules]` table names the rules a project runs. `enable` runs only those
  named, `disable` runs everything else, and a name that no configured lint
  reports is an error rather than a filter that quietly admits everything.
- A `[rules.options."<rule>"]` table gives one rule the names it cannot read
  off the source, such as the attributes that mark a system boundary. A
  value is a list of names, and a rule reads its own entry in `configure`.
  Plugins built for an earlier protocol no longer load, because the method is
  new on `LintPass`.
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

- The values a plugin exchanges with whisker are laid out by [stabby][stabby]:
  `Diagnostic`, `Span`, `FilePath`, `Location`, `Suggestion`, `RuleId`,
  `Severity`, `RuleOptions`, and `RuleOption`. Their layout no longer depends
  on the compiler, which is the first step toward loading a plugin that
  another rustc built. A span names its file through `FilePath`, and
  `Span::file_arc` is now `Span::file_path`. `RuleOptions::names` returns
  owned names.
- The decorations a plugin reads are laid out by stabby too: `ResolvedType`,
  `FnSignature`, `AdtFlags`, and `ImportSource`, with the types they hold. A
  `Decoration` must now be `IStable`, so a decoration std would lay out is a
  compile error rather than a plugin that reads the wrong bytes.
- A node crosses the boundary laid out by stabby, and it reads decorations
  through a call into whisker rather than by walking whisker's map. The
  boundary fingerprint now names only the types a pass receives and
  returns.
- A pass's methods are `extern "C"` and hand back a value, so a call into a
  pass no longer depends on the calling convention of the compiler that
  built it. A panic inside a rule comes back as a value too, and whisker
  reports the file and node it was checking instead of the process dying.
  The protocol is 5, and a plugin built for an earlier one no longer loads.
- A plugin declares the rules it reports, which is what a `[rules]` name is
  checked against.

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
[stabby]: https://crates.io/crates/stabby
