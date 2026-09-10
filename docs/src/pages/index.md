# Whisker

Whisker is a linting platform built on tree-sitter. It lints Rust today.

Whisker ships no rules of its own. A project brings its own, either a crate in
the repository or a repository of shared rules pinned to a commit. Rules can be
as specific as your codebase is: the ones a language's own linter would never
carry, because only your project wants them.

## Install

```bash
curl -LsSf https://aonyx-ai.github.io/whisker/install.sh | sh
```

[Install Whisker][installation] covers Linux, macOS, GitHub Actions, and a
build from source.

## Where to start

- **[Quick start][quick-start]**  
  Install Whisker, write a rule, and check a project.
- **[Write a rule][custom-lints]**  
  Hook a node kind, test it, and give it options.
- **[Configuration][configuration]**  
  Every key of `.config/whisker.toml`.
- **[How Whisker works][how-it-works]**  
  Why syntax and semantics stay apart, and what that buys.

[configuration]: /docs/reference/configuration
[custom-lints]: /authoring/how-to/write-a-rule
[how-it-works]: /docs/explanation/how-whisker-works
[installation]: /docs/how-to/install
[quick-start]: /docs/tutorials/quick-start
