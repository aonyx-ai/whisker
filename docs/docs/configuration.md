---
sidebar_position: 3
---

# Configuration

Whisker reads its configuration from `.config/whisker.toml`:

```toml
ignore = ["/examples/", "crates/whisker-rust/tests/fixtures/"]
```

## Where whisker looks for it

The search starts at the path you check and climbs until it finds that file, or
failing that, a `.git` directory. `whisker check src/` still finds the
configuration at the top of your repository, and a run inside a repository
never reads a file from outside it. A directory with a configuration file of
its own is its own project, so one repository can hold several. A directory
that is neither is still a valid target: whisker checks it and applies no
patterns.

## Ignoring paths

`ignore` holds gitignore-syntax patterns. Use it for test fixtures, vendored
sources, and other code that git tracks on purpose. The patterns anchor at the
project directory, the one that holds `.config`, and they behave as they would
in a `.gitignore` written there. `crates/app/generated/` names one directory
relative to that root, `examples/` matches at any depth, and `/examples/`
matches only at the root.

Whisker rejects keys it does not recognize, so a typo is an error.
