#!/usr/bin/env python3
"""Write edited sections back into the Whisker docs pages.

Reads the shape parse_docs.py emits, on stdin or from a path, and rewrites
each file whose sections carry edits. A section nobody touched is emitted from
its verbatim `original`, so an unedited file is byte-identical to what it was;
only an edited file is rewritten at all.

Pass --check to compare against what is on disk instead of writing.
"""

import json
import pathlib
import re
import sys

DOCS = pathlib.Path(__file__).resolve().parent.parent
WIDTH = 80
RESOLVED = re.compile(r"^\[[^\]]+\]:\s*\S")
UNRESOLVED = []


def wrap(text, prefix=""):
    """Wraps one paragraph to the 80 column limit markdownlint enforces."""
    words = text.split()
    lines, current = [], prefix.rstrip()
    for word in words:
        candidate = f"{current} {word}" if current.strip() else f"{prefix}{word}"
        if current.strip() and len(candidate) > WIDTH:
            lines.append(current)
            current = f"{prefix}{word}"
            continue
        current = candidate
    if current.strip():
        lines.append(current)
    return lines


def brief_comment(brief):
    lines = ["<!--"]
    for prompt in brief:
        key, prose = prompt.get("key", ""), prompt.get("prose", "")
        lines.extend(wrap(f"{key}: {prose}" if key else prose))
    lines.append("-->")
    return "\n".join(lines)


def meta_comment(meta):
    lines = ["<!--"]
    for note in meta:
        lines.extend(wrap(note))
    lines.append("-->")
    return "\n".join(lines)


def render_section(section):
    heading = section["heading"].strip()
    anchor = section.get("anchor", "").strip()
    brief = section.get("brief") or []
    body = section.get("body", "").strip("\n")

    blocks = []
    if heading:
        blocks.append(f"## {heading} {anchor}".rstrip() if anchor else f"## {heading}")
    if brief and section.get("status") != "done":
        blocks.append(brief_comment(brief))
    if body:
        blocks.append(body)
    return "\n\n".join(blocks) + "\n\n" if blocks else ""


def render_file(record, sections):
    sections = sorted(sections, key=lambda item: item["order"])
    restructure = any(
        item["kind"] == "proposed" and item.get("dirty") for item in sections
    )
    live = [
        item
        for item in sections
        if item["kind"] != "proposed" or restructure or item.get("body", "").strip()
    ]

    touched = any(item.get("dirty") for item in sections) or record.get("dirty")
    if not touched:
        return record["originalText"]

    intro = next((item for item in live if item["kind"] == "intro"), None)
    head = []
    if record.get("prelude"):
        head.append(record["prelude"])
    if record.get("title"):
        head.append(f"# {record['title']}")

    body = []
    keep_preamble = (
        not restructure
        and (intro is None or not intro.get("dirty"))
        and not record.get("dirty")
    )
    if keep_preamble:
        preamble = record.get("originalPreamble", "").strip("\n")
        if preamble:
            body.append(preamble + "\n\n")
    else:
        meta = list(record.get("meta") or [])
        if meta:
            body.append(meta_comment(meta) + "\n\n")
        if intro is not None and intro.get("brief") and not restructure:
            body.append(brief_comment(intro["brief"]) + "\n\n")
        if intro is not None and intro.get("body", "").strip():
            body.append(intro["body"].strip("\n") + "\n\n")

    for section in live:
        if section["kind"] == "intro":
            continue
        settled = section.get("status") == "done" and (section.get("brief") or [])
        if not section.get("dirty") and not settled and section.get("original"):
            body.append(section["original"].strip("\n") + "\n\n")
            continue
        body.append(render_section(section))

    text = "\n\n".join(head)
    if head:
        text += "\n\n"
    text += "".join(body)

    links, open_refs = [], []
    for line in (record.get("links") or "").split("\n"):
        line = line.strip()
        if not line:
            continue
        if RESOLVED.match(line):
            links.append(line)
        else:
            open_refs.append(line)
    if open_refs:
        UNRESOLVED.append((record["path"], open_refs))
    if links:
        text = text.rstrip("\n") + "\n\n" + "\n".join(links) + "\n"
    else:
        text = text.rstrip("\n") + "\n"
    return text


def main():
    argv = [item for item in sys.argv[1:] if not item.startswith("--")]
    check = "--check" in sys.argv
    payload = json.loads(pathlib.Path(argv[0]).read_text() if argv else sys.stdin.read())

    by_file = {}
    for section in payload["sections"]:
        by_file.setdefault(section["fileId"], []).append(section)

    changed, identical, mismatched = [], [], []
    for record in payload["files"]:
        text = render_file(record, by_file.get(record["id"], []))
        path = DOCS / record["path"]
        if text == record["originalText"]:
            identical.append(record["path"])
            continue
        if check:
            mismatched.append(record["path"])
            continue
        path.write_text(text)
        changed.append(record["path"])

    for path, open_refs in UNRESOLVED:
        for line in open_refs:
            print(f"no target yet, left out of {path}: {line}", file=sys.stderr)

    if check:
        for path in mismatched:
            print(f"differs: {path}")
        print(f"{len(identical)} identical, {len(mismatched)} differ")
        return 1 if mismatched else 0

    for path in changed:
        print(f"wrote: {path}")
    print(f"{len(changed)} written, {len(identical)} unchanged")
    return 0


sys.exit(main())
