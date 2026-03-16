# Aonyx project template

This document describes the standard project scaffolding used across Aonyx
repositories. When creating a new project, copy these files and replace the
placeholders.

## Placeholders

| Placeholder        | Example                               | Where it appears                                            |
| ------------------ | ------------------------------------- | ----------------------------------------------------------- |
| `{{project}}`      | `whisker`                             | CLAUDE.md, README.md, justfile, Cargo.toml, labelflair.toml |
| `{{description}}`  | `A tree-sitter based AST linter`      | README.md, Cargo.toml                                       |
| `{{rust-version}}` | `1.94.0`                              | rust-toolchain.toml                                         |
| `{{msrv}}`         | `1.85.0`                              | Cargo.toml `rust-version` field                             |
| `{{github-url}}`   | `https://github.com/aonyx-ai/whisker` | CLAUDE.md, Cargo.toml                                       |

## Files

### Identical across all projects (copy as-is)

```text
.config/nextest.toml
.editorconfig
.prettierrc.json
.prettierignore
.rustfmt.toml
.yamllint.yaml
.markdownlint.yaml
.markdownlintignore
.pre-commit-config.yaml
deny.toml
LICENSE-APACHE
LICENSE-MIT
zizmor.yml
.github/workflows/ci.yml
.github/workflows/labelflair.yml
.github/workflows/publish.yml
.github/release.yml
.github/renovate.json5
.claude/settings.local.json
```

### Require `{{project}}` substitution only

- **justfile** — project name in recipe comments (e.g. "Check that {{project}}
  builds with the latest dependencies")
- **CHANGELOG.md** — no substitution needed, starts with `[Unreleased]`

### Require multiple substitutions

#### `.taplo.toml`

Only the `exclude` list changes. Start with just `.flox/**` and add
project-specific paths as needed (e.g. test fixture TOML files).

#### `Cargo.toml`

```toml
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "Apache-2.0 OR MIT"
repository = "{{github-url}}.git"
rust-version = "{{msrv}}"

[workspace.dependencies]
# Add as needed
```

#### `rust-toolchain.toml`

```toml
[toolchain]
channel = "{{rust-version}}"
components = ["clippy", "rustfmt"]
```

#### `.gitignore`

Always includes:

```text
debug/
target/
**/*.rs.bk
*.pdb
.tracey/
```

Add project-specific entries as needed.

#### `.github/labelflair.toml`

The `good-first-issue` label, `C-` group, and `R-` group are identical. Only
the `A-` (area) group changes per project:

```toml
[[group]]
prefix = "A-"
colors = { tailwind = "lime" }
labels = [
  { name = "{{project}}", description = "An issue related to {{project}}" },
  { name = "github-actions", description = "An issue related to GitHub Actions" },
]
```

Add more area labels as crates are added.

#### `README.md`

Follows this structure:

```markdown
# {{Project}}

{{description}}.

## Status / Quick start

## License (dual Apache-2.0 / MIT)

## Contribution (DCO clause)
```

#### `CLAUDE.md`

The document has two kinds of content:

**Shared across all projects** (the bulk of the file):

- For humans / for LLMs sections (swap project name and GitHub URL)
- Continuous improvement, working style
- Philosophy (correctness, UX, incrementalism, production-grade)
- Rust style guide (edition, modules, memory, dependencies, type system,
  coding patterns, error handling, testing, documentation)
- Markdown conventions
- Git conventions (commit messages, commit quality, pull requests)
- Acknowledgments

**Project-specific sections** (add between "Development environment" and
"Quick reference"):

- Project structure (`crates/` layout)
- Conventions (project-specific patterns and idioms)
- Architecture (key design decisions)
- Key dependencies (beyond the standard set)

## Setup checklist

1. Create the GitHub repository
2. Copy all identical files
3. Create templated files with substitutions
4. Write project-specific README and CLAUDE.md sections
5. Run `git init && git remote add origin <url>`
6. Create initial commit
7. Push to GitHub
8. Run `pre-commit install` (requires [pre-commit][pre-commit])
9. Set up Flox environment (`flox init`, install dev tools)

[pre-commit]: https://pre-commit.com
