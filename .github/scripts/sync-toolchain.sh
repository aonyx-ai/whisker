#!/usr/bin/env bash
#
# Syncs every rust-toolchain.toml to the nightly date required by the
# clippy_utils version in Cargo.lock. The nightly date is extracted from the
# clippy_utils crate README on crates.io.
#
# The version comes from the lockfile rather than Cargo.toml because the
# manifest holds a range, and the range is not what gets compiled. A caret
# range of "0.1.98" happily resolves to 0.1.99, which needs a different
# nightly than the one the manifest implies, so reading the manifest can pin
# a toolchain that cannot build the code that is actually selected.
#
# Usage: sync-toolchain.sh [--check]
#
# With --check, exits 1 if any toolchain is out of sync (useful in CI).
# Without --check, updates the toolchain files in place.

set -euo pipefail

# Every toolchain file in the tree, discovered rather than listed, so that
# adding or removing a crate with its own pinned channel needs no edit here.
# Read in a loop rather than with `mapfile`, which macOS's bash 3.2 lacks.
toolchain_files=()
while IFS= read -r file; do
  toolchain_files+=("$file")
done < <(git ls-files '*rust-toolchain.toml')

check_only=false
if [ "${1:-}" = "--check" ]; then
  check_only=true
fi

version=$(
  awk '/^name = "clippy_utils"$/ { getline; gsub(/^version = "|"$/, ""); print; exit }' \
    Cargo.lock
)

# clippy_utils is what ties this repository to a particular nightly. Once the
# tree-sitter platform replaces Dylint it stops being a dependency, and with
# nothing left to pin the toolchain against there is nothing to check.
if [ -z "$version" ]; then
  echo "clippy_utils is not a dependency; nothing to sync"
  exit 0
fi

if [ "${#toolchain_files[@]}" -eq 0 ]; then
  echo "No rust-toolchain.toml found; nothing to sync"
  exit 0
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
