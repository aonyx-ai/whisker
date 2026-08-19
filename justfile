# Run all recipes inside the Flox environment
set shell := ["flox", "activate", "--", "sh", "-cu"]

[private]
default:
    @just --list

# Run a subset of checks as pre-commit hooks
pre-commit-inner:
    #!/usr/bin/env -S parallel --shebang --ungroup --jobs {{ num_cpus() }}
    just prettier true
    just format-toml true
    just format-rust true
    just lint-github-actions
    just lint-markdown
    just lint-rust
    just lint-yaml
    just test-rust

pre-commit:
    just pre-commit-inner

# Check that dependencies have compatible open-source licenses and trusted sources
check-dependencies:
    cargo deny check bans licenses sources

# Check that a lint crate's view of whisker-rust builds without the provider
check-features:
    cargo check -p whisker-rust --no-default-features

# Format JSON files
format-json fix="false": (prettier fix "{json,json5}")

# Format Markdown files
format-markdown fix="false": (prettier fix "md")

# Format Rust files, including the example plugin outside the workspace
format-rust fix="false":
    cargo fmt -- --unstable-features {{ if fix != "true" { "--check" } else { "" } }}
    cd examples/custom_lint && cargo fmt -- --unstable-features {{ if fix != "true" { "--check" } else { "" } }}

# Format TOML files
format-toml fix="false":
    taplo fmt {{ if fix != "true" { "--diff" } else { "" } }}

# Format YAML files
format-yaml fix="false": (prettier fix "{yaml,yml}")

# Lint GitHub Actions workflows
lint-github-actions:
    zizmor -p .

# Lint Markdown files
lint-markdown:
    markdownlint --ignore-path .gitignore "**/*.md"

# Lint Rust files, including the example plugin outside the workspace
lint-rust:
    cargo clippy --all-targets --all-features -- -D warnings
    cd examples/custom_lint && cargo clippy --all-targets --all-features -- -D warnings

# Lint TOML files
lint-toml:
    taplo check

# Lint YAML files
lint-yaml:
    yamllint .

# Auto-format files with prettier
[private]
prettier fix="false" extension="*":
    prettier {{ if fix == "true" { "--write" } else { "--list-different" } }} --ignore-unknown "**/*.{{ extension }}"

# Run the tests
test-rust:
    cargo nextest run --all-features
    cargo test --doc --all-features

# Run the example plugin's tests, which sit outside the workspace
#
# The package is its own workspace, so nextest would not find the profile
# CI selects without being pointed at this repository's configuration.
test-example-lint:
    cd examples/custom_lint && cargo nextest run --config-file ../../.config/nextest.toml
