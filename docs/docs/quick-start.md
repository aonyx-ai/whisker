---
sidebar_position: 0
---

# Quick start

This page takes you from nothing to a checked project. It assumes you have a
Cargo workspace to point whisker at.

## Install whisker

Download the archive for your platform from the [releases page][releases],
check it against the digest published beside it, and unpack it:

```bash
shasum -a 256 -c whisker-0.1.0-rc.3-aarch64-apple-darwin.tar.gz.sha256
tar -xzf whisker-0.1.0-rc.3-aarch64-apple-darwin.tar.gz
```

The archive unpacks to a directory that holds the binary, both licenses, and
the README. Move `whisker` to a directory on your `PATH`.

[Installation](/docs/installation) covers the other ways to install whisker,
and which one to choose.

## Choose the rules to run

Whisker ships no rules of its own, so a project with no configuration runs no
rules. Write `.config/whisker.toml` at the top of your repository and name a
source of rules:

```toml
[[lints]]
git = "https://github.com/aonyx-ai/whisker-aonyx-rules"
rev = "ffd02b34a39900a84045ff7bc0130885bf9f5732"
```

A repository source is pinned to one commit, written out in full. A branch or
a tag is whatever the remote points it at today, so the same configuration
would run different rules on different days.

The commit above is the one whisker's own repository pins. Pick the commit you
want; moving the pin is how a project adopts a new rule.

## Check the project

```bash
whisker check .
```

The first run builds the rules, which takes as long as any Rust compilation.
Later runs reuse cargo's cache.

A rule has to be built by the same toolchain as the whisker that loads it. If
the run stops with a handshake error, [custom lints](/docs/custom-lints)
explains what to rebuild.

## Next steps

- [Checking a project](/docs/checking-a-project) — which files whisker
  inspects, and what makes a run fail.
- [Configuration](/docs/configuration) — the rest of `.config/whisker.toml`,
  including how to ignore paths.
- [Custom lints](/docs/custom-lints) — run a rule at a time, or write your own.

[releases]: https://github.com/aonyx-ai/whisker/releases
