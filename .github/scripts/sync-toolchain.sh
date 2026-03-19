#!/usr/bin/env bash
#
# Syncs rust-toolchain.toml to the nightly date required by the clippy_utils
# version in Cargo.toml. The nightly date is extracted from the clippy_utils
# crate README on crates.io.
#
# Usage: sync-toolchain.sh [--check]
#
# With --check, exits 1 if the toolchain is out of sync (useful in CI).
# Without --check, updates rust-toolchain.toml in place.

set -euo pipefail

check_only=false
if [ "${1:-}" = "--check" ]; then
  check_only=true
fi

version=$(sed -n 's/^clippy_utils *= *"\([^"]*\)"/\1/p' Cargo.toml)
if [ -z "$version" ]; then
  echo "No clippy_utils version found in Cargo.toml" >&2
  exit 1
fi

nightly=$(
  curl -sL -H "User-Agent: whisker-ci" \
    "https://crates.io/api/v1/crates/clippy_utils/$version/download" \
    | tar xz -O "clippy_utils-$version/README.md" \
    | grep -oE 'nightly-[0-9]{4}-[0-9]{2}-[0-9]{2}' \
    | head -1
)
if [ -z "$nightly" ]; then
  echo "Could not find nightly date in clippy_utils $version README" >&2
  exit 1
fi

current=$(sed -n 's/^channel *= *"\(nightly-[0-9-]*\)"/\1/p' rust-toolchain.toml)

if [ "$current" = "$nightly" ]; then
  echo "rust-toolchain.toml already up to date ($nightly)"
  exit 0
fi

if [ "$check_only" = true ]; then
  echo "rust-toolchain.toml is out of sync: has $current, expected $nightly" >&2
  exit 1
fi

sed -i'' -e "s/$current/$nightly/" rust-toolchain.toml
echo "Updated rust-toolchain.toml: $current -> $nightly"
