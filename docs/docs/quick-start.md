---
sidebar_position: 0
---

# Quick start

This page takes you from nothing to a checked project. It assumes you have a
Cargo workspace to point whisker at.

## Install whisker

```bash
curl -LsSf https://aonyx-ai.github.io/whisker/install.sh | sh
```

The script installs to `~/.local/bin`. Put that on your `PATH` if it is not
there already.

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
