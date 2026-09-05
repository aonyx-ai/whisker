# Run all recipes inside the Flox environment
set shell := ["flox", "activate", "--", "sh", "-cu"]

# Commands to build and serve the documentation site
mod docs

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

# Format Rust files
format-rust fix="false":
    #!/usr/bin/env -S bash -euo pipefail
    # The plugin packages sit outside the workspace, so a run at the root
    # never reaches them. They are Rust like everything else and drift the
    # moment nothing checks them.
    for package in . examples/custom_lint crates/whisker-rust/tests/fixtures/lints/decoration_probes; do
        (cd "${package}" && cargo fmt -- --unstable-features {{ if fix != "true" { "--check" } else { "" } }})
    done

# Format TOML files
format-toml fix="false":
    taplo fmt {{ if fix != "true" { "--diff" } else { "" } }}

# Format YAML files
format-yaml fix="false": (prettier fix "{yaml,yml}")

# Lint GitHub Actions workflows
lint-github-actions:
    zizmor -p .

# `--ignore-path` replaces the `.markdownlintignore` that markdownlint reads by
# default, so the recipe passes both files. `.gitignore` covers what git ignores
# at the root, `.markdownlintignore` what only this linter skips.

# Lint Markdown files
lint-markdown:
    markdownlint --ignore-path .gitignore --ignore-path .markdownlintignore "**/*.md"

# Lint Rust files
lint-rust:
    cargo clippy --all-targets --all-features -- -D warnings

# Lint TOML files
lint-toml:
    taplo check

# Lint YAML files
lint-yaml:
    yamllint .

# Assemble a release archive of whisker for one target
package-whisker version target:
    #!/usr/bin/env -S bash -euo pipefail
    # The archive holds the binary, both licenses, and the README. A sidecar
    # beside it carries the SHA-256 digest, so a download can be verified
    # with `shasum -a 256 -c`. The recipe runs outside Flox, because the
    # release runners have rustup and Flox installs on x86_64 Linux alone.
    version="{{ version }}"
    version="${version#v}"
    target="{{ target }}"

    # The tag names the version, and `Cargo.toml` names it too. A tag that
    # disagrees would ship an archive whose name promises a version the
    # binary does not report, so the disagreement stops the release here.
    #
    # A prerelease tag such as `v0.1.0-rc.1` carries a suffix that the
    # crate version never holds, and the binary reports `0.1.0` whatever
    # the suffix says. The comparison therefore drops the suffix, while
    # the archive keeps the whole tag in its name.
    release="${version%%-*}"
    pinned="$(cargo pkgid -p whisker)"
    pinned="${pinned##*#}"
    pinned="${pinned##*@}"
    if [ "${release}" != "${pinned}" ]; then
        echo "the tag names version ${release}, but Cargo.toml holds ${pinned}" >&2
        exit 1
    fi

    cargo build --release --locked -p whisker --target "${target}"
    whisker="target/${target}/release/whisker"
    "${whisker}" --version

    # The archive's name promises a platform, and the binary decides which
    # prebuilt lints it asks a publisher for. A disagreement would send
    # everyone who unpacks this archive looking for artifacts that were
    # never published under that name, so the two are compared here.
    tag="$("${whisker}" abi)"
    if [ "${tag#*-}" != "${target}" ]; then
        echo "the binary is built for ${tag#*-}, but the archive names ${target}" >&2
        exit 1
    fi

    name="whisker-${version}-${target}"
    rm -rf "dist/${name}"
    mkdir -p "dist/${name}"
    cp "target/${target}/release/whisker" "dist/${name}/"
    cp LICENSE-APACHE LICENSE-MIT README.md "dist/${name}/"

    # Short flags, because macOS ships bsdtar and its support for the
    # GNU long spellings is not something a release should rely on.
    tar -czf "dist/${name}.tar.gz" -C dist "${name}"
    rm -rf "dist/${name}"
    (cd dist && shasum -a 256 "${name}.tar.gz" > "${name}.tar.gz.sha256")

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

# Run the decoration probes' tests, which sit outside the workspace
#
# The probes stand in for rules inside whisker-rust's provider tests. They
# are excluded from the workspace for the same reason every plugin is, so
# nextest needs pointing at this repository's profile.
test-fixture-lint:
    cd crates/whisker-rust/tests/fixtures/lints/decoration_probes && cargo nextest run --config-file ../../../../../../.config/nextest.toml

# Check this repository with the rules it configures
#
# This is the end-to-end run: whisker resolves every configured lint source,
# builds it, completes the handshake, and reports on its own sources. It was
# once an integration test, but a test that reaches the network belongs in a
# recipe a person can choose to run, not in `cargo test`.
check-self:
    #!/usr/bin/env -S bash -euo pipefail
    cargo build --release -p whisker

    # The rules repository pins a toolchain of its own, and whisker's may
    # move ahead of it. Rustup would then build those rules with their
    # toolchain, and the handshake would refuse plugins that whisker had
    # just built. Whisker passes its environment to the cargo it runs, so
    # naming the toolchain here builds them with this repository's.
    #
    # This belongs to the recipe and not to whisker. Whisker has no business
    # overriding the toolchain a plugin author chose; the two pins are
    # coupled only because both repositories are ours.
    RUSTUP_TOOLCHAIN="$(rustup show active-toolchain | cut -d" " -f1)" \
        ./target/release/whisker check .
