#!/usr/bin/env python3
"""Overlay the desk's edits onto a fresh parse of the docs pages.

Usage: merge_docs.py <seed.json> <export dir> > edited.json

The export dir is what `ArtifactData` writes with `out_dir`: one JSON file per
document under `files/` and `sections/`. The seed comes from parse_docs.py run
against the working tree, so the verbatim originals are always current and an
untouched section still round-trips byte for byte.
"""

import json
import pathlib
import sys

SECTION_FIELDS = ("heading", "anchor", "body", "status", "dirty")
FILE_FIELDS = ("title", "links", "dirty")


def load(directory, collection):
    root = pathlib.Path(directory) / collection
    documents = {}
    for path in sorted(root.glob("*.json")):
        payload = json.loads(path.read_text())
        payload = payload.get("data", payload)
        documents[payload.get("id", path.stem)] = payload
    return documents


def main():
    seed_path, export = sys.argv[1], sys.argv[2]
    seed = json.loads(pathlib.Path(seed_path).read_text())

    files = load(export, "files")
    sections = load(export, "sections")

    missing, edited = [], 0
    for record in seed["files"]:
        live = files.get(record["id"])
        if live is None:
            missing.append("files/" + record["id"])
            continue
        for field in FILE_FIELDS:
            if field in live:
                record[field] = live[field]
        edited += 1 if record.get("dirty") else 0

    for record in seed["sections"]:
        live = sections.get(record["id"])
        if live is None:
            missing.append("sections/" + record["id"])
            continue
        for field in SECTION_FIELDS:
            if field in live:
                record[field] = live[field]
        edited += 1 if record.get("dirty") else 0

    orphans = set(sections) - {record["id"] for record in seed["sections"]}

    json.dump(seed, sys.stdout)
    for path in sorted(missing):
        print(f"missing from the export: {path}", file=sys.stderr)
    for path in sorted(orphans):
        print(f"in the desk but not in the tree: sections/{path}", file=sys.stderr)
    print(f"{edited} documents carry edits", file=sys.stderr)


main()
