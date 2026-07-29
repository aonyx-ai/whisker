#!/usr/bin/env bash
#
# Syncs every rust-toolchain.toml to the nightly date required by the
# clippy_utils version in Cargo.toml. The nightly date is extracted from the
# clippy_utils crate README on crates.io.
#
# Usage: sync-toolchain.sh [--check]
#
# With --check, exits 1 if any toolchain is out of sync (useful in CI).
# Without --check, updates the toolchain files in place.

set -euo pipefail

toolchain_files=(rust-toolchain.toml crates/whisker/rust-toolchain.toml)

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

out_of_sync=false

for file in "${toolchain_files[@]}"; do
  if [ ! -f "$file" ]; then
    echo "$file does not exist; update toolchain_files in $0" >&2
    exit 1
  fi

  current=$(sed -n 's/^channel *= *"\(nightly-[0-9-]*\)"/\1/p' "$file")
  if [ -z "$current" ]; then
    echo "No nightly channel found in $file" >&2
    exit 1
  fi

  if [ "$current" = "$nightly" ]; then
    echo "$file already up to date ($nightly)"
    continue
  fi

  if [ "$check_only" = true ]; then
    echo "$file is out of sync: has $current, expected $nightly" >&2
    out_of_sync=true
    continue
  fi

  # BSD sed reads the argument after `-i` as the backup suffix, so an explicit
  # suffix plus a delete is the only spelling that behaves the same on both
  # BSD and GNU sed.
  sed -i.bak -e "s/$current/$nightly/" "$file"
  rm -f "$file.bak"
  echo "Updated $file: $current -> $nightly"
done

if [ "$out_of_sync" = true ]; then
  exit 1
fi
