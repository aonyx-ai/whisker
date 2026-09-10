# Whisker

Whisker is a language-agnostic linting platform that lets you build
hyper-specific lints.

With the dawn of agentic coding, the software development world has (finally)
become interested in stricter guardrails. Language-specific linting tools are
limited to community-wide best practice rules, but there are probably more
specific rules in your projects or organization. Whisker aims to let you build
lints against those rules, however weird they may be.

Installing Whisker by itself isn't very valuable, as it _does not_ ship any
lints. Follow the [quick start][quick-start] to set up the tooling and build
your first lint.

Whisker does not currently attempt to promise any kind of linting performance.
We may optimize Whisker later, but for now we're focused on covering all lints
you may want to build.

## Install

```bash
curl -LsSf https://aonyx-ai.github.io/whisker/install.sh | sh
```

[Installation](/docs/installation) covers Linux, macOS, GitHub Actions, and a
build from source.

## Where to start

- **[Quick start][quick-start]**  
  Install Whisker, point it at a set of rules, and check a project.

[quick-start]: /docs/quick-start
