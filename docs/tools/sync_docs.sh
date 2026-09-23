#!/bin/bash
# Pull the docs desk's edits into the working tree.
#
# Export the desk's `files` and `sections` collections to $PULL first, with
# ArtifactData's list action and out_dir=$PULL, then run this. Nothing is
# written to the desk, so a sync never races a live editor.
#
# Pass --check to see what would change without writing anything.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
WORK="${WORK:-${TMPDIR:-/tmp}/docs-desk}"
PULL="${PULL:-$WORK/pull}"
mkdir -p "$WORK"

python3 "$HERE/parse_docs.py" > "$WORK/seed.json"
python3 "$HERE/merge_docs.py" "$WORK/seed.json" "$PULL" > "$WORK/edited.json"
python3 "$HERE/write_docs.py" "$@" "$WORK/edited.json"
