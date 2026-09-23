#!/usr/bin/env python3
"""Parse every Whisker docs page into editable sections.

Emits JSON on stdout as {"files": [...], "sections": [...]}, keyed by stable
ids, so the desk can be seeded from it and the result written back by
write_docs.py. Every page carries an introduction section at the fixed id
`<slug>__intro`, so adding one never renumbers the sections below it.
"""

import json
import pathlib
import re
import sys

DOCS = pathlib.Path(__file__).resolve().parent.parent

FILES = [
    "docs/tutorials/quick-start.mdx",
    "docs/how-to/install.md",
    "docs/how-to/github-actions.md",
    "docs/how-to/shared-rules.md",
    "docs/how-to/adopt-rules-gradually.md",
    "docs/how-to/troubleshooting.md",
    "docs/reference/configuration.md",
    "docs/reference/command-line.md",
    "docs/reference/file-discovery.md",
    "docs/reference/prebuilt-archives.md",
    "docs/reference/environment-variables.md",
    "docs/reference/cache-layout.md",
    "docs/reference/platforms.md",
    "docs/explanation/how-whisker-works.md",
    "docs/explanation/package-and-abi-versioning.md",
    "docs/explanation/pinning-and-trust.md",
    "authoring/how-to/write-a-rule.md",
    "authoring/reference/api.md",
    "authoring/reference/prebuilt-archives.md",
    "authoring/explanation/plugin-boundary.md",
    "src/pages/index.md",
    "README.md",
]

GROUPS = {
    "docs/tutorials": ("Usage", "Tutorials"),
    "docs/how-to": ("Usage", "How-to guides"),
    "docs/reference": ("Usage", "Reference"),
    "docs/explanation": ("Usage", "Explanation"),
    "authoring/how-to": ("Authoring", "How-to guides"),
    "authoring/reference": ("Authoring", "Reference"),
    "authoring/explanation": ("Authoring", "Explanation"),
    "src/pages": ("Site", "Pages"),
    "": ("Site", "Repository"),
}

COMMENT = re.compile(r"<!--(.*?)-->", re.DOTALL)
HEADING = re.compile(r"^(##+) +(.*)$", re.MULTILINE)
PROMPT = re.compile(r"^([A-Za-z][^:.\n]{0,48}): +(.+)$", re.DOTALL)
LINKDEF = re.compile(r"^\[[^\]]+\]: +\S+\s*$")


def slug(path):
    return path.replace("/", "-").replace(".", "-")


def group_for(path):
    parent = str(pathlib.PurePosixPath(path).parent)
    if parent == ".":
        parent = ""
    return GROUPS.get(parent, ("Site", "Other"))


def split_prompts(comment, level):
    """Splits an outline comment into meta notes and writing prompts.

    Paragraphs are separated by blank lines, by a line matching `key: `, and by
    a sentence end, because an outline wraps one prompt over several lines but
    never starts a new one mid-sentence. A page comment keeps its goal,
    non-goal, and plain notes as meta, and turns any `key: prose` paragraph
    into a prompt for a section the page does not have yet.
    """
    meta, prompts = [], []
    lines = comment.strip("\n").split("\n")
    buffer = []

    def flush():
        if not buffer:
            return
        text = " ".join(part.strip() for part in buffer).strip()
        buffer.clear()
        if not text:
            return
        match = PROMPT.match(text)
        key = match.group(1).strip() if match else ""
        prose = match.group(2).strip() if match else text
        if key in ("goal", "non-goal"):
            meta.append(text)
            return
        if level == "page" and not match:
            meta.append(text)
            return
        prompts.append({"key": key, "prose": " ".join(prose.split())})

    for line in lines:
        stripped = line.strip()
        if not stripped:
            flush()
            continue
        if buffer and (PROMPT.match(stripped) or buffer[-1].rstrip().endswith(".")):
            flush()
        buffer.append(line)
    flush()
    return meta, prompts


def take_comment(body):
    """Removes the leading outline comment from a section body."""
    match = COMMENT.search(body)
    if not match or body[: match.start()].strip():
        return "", body.strip("\n")
    rest = body[: match.start()] + body[match.end() :]
    return match.group(1), rest.strip("\n")


def take_links(text):
    """Removes the trailing block of link reference definitions from a file."""
    lines = text.rstrip("\n").split("\n")
    cut = len(lines)
    while cut and (LINKDEF.match(lines[cut - 1]) or not lines[cut - 1].strip()):
        cut -= 1
    links = [line for line in lines[cut:] if line.strip()]
    if not links:
        return text, ""
    return "\n".join(lines[:cut]).rstrip("\n") + "\n", "\n".join(links)


def heading_title(text):
    anchor = ""
    match = re.search(r"\s*(\{#[^}]+\})\s*$", text)
    if match:
        anchor = match.group(1)
        text = text[: match.start()]
    return text.strip(), anchor


def parse(path):
    source = (DOCS / path).read_text()
    text = source

    prelude = ""
    if path.endswith(".mdx"):
        lines = text.split("\n")
        cut = 0
        for index, line in enumerate(lines):
            if line.startswith("import ") or not line.strip():
                cut = index + 1
                continue
            break
        prelude = "\n".join(lines[:cut]).strip("\n")
        text = "\n".join(lines[cut:])

    text, links = take_links(text)

    title = ""
    if text.lstrip().startswith("# "):
        first, _, text = text.lstrip().partition("\n")
        title = first[2:].strip()

    cuts = [
        (match.start(), match.group(2))
        for match in HEADING.finditer(text)
        if len(match.group(1)) == 2
    ]
    preamble = text[: cuts[0][0]] if cuts else text
    originals = [preamble]
    page_comment, intro_body = take_comment(preamble)
    page_meta, page_prompts = split_prompts(page_comment, "page")

    sections = []
    if intro_body.strip() or not cuts or page_prompts:
        sections.append(
            {
                "heading": "",
                "anchor": "",
                "brief": [],
                "body": intro_body,
                "kind": "intro",
                "original": originals[0],
            }
        )

    for index, (start, raw) in enumerate(cuts):
        end = cuts[index + 1][0] if index + 1 < len(cuts) else len(text)
        originals.append(text[start:end])
        _, _, block = text[start:end].partition("\n")
        comment, body = take_comment(block)
        _, prompts = split_prompts(comment, "section")
        heading, anchor = heading_title(raw)
        sections.append(
            {
                "heading": heading,
                "anchor": anchor,
                "brief": prompts,
                "body": body,
                "kind": "section",
                "original": originals[index + 1],
            }
        )

    if not cuts and page_prompts:
        for prompt in page_prompts:
            key = prompt["key"]
            sections.append(
                {
                    "heading": key[0].upper() + key[1:],
                    "anchor": "",
                    "brief": [prompt],
                    "body": "",
                    "kind": "proposed",
                    "original": "",
                }
            )
    elif page_prompts:
        sections[0]["brief"] = page_prompts

    area, category = group_for(path)
    record = {
        "id": slug(path),
        "path": path,
        "title": title,
        "prelude": prelude,
        "links": links,
        "meta": page_meta,
        "area": area,
        "category": category,
        "originalText": source,
        "originalPreamble": preamble,
    }

    rows = []
    order = 0
    for section in sections:
        if section["kind"] == "intro":
            rows.append(
                {
                    "id": f"{slug(path)}__intro",
                    "_intro": True,
                    "file": path,
                    "fileId": slug(path),
                    "area": area,
                    "category": category,
                    "order": -1,
                    "heading": "",
                    "anchor": "",
                    "brief": section["brief"],
                    "body": section["body"],
                    "kind": "intro",
                    "original": section["original"],
                    "status": "drafted" if section["body"].strip() else "todo",
                }
            )
            continue

        rows.append(
            {
                "id": f"{slug(path)}__{order}",
                "_intro": False,
                "file": path,
                "fileId": slug(path),
                "area": area,
                "category": category,
                "order": order,
                "heading": section["heading"],
                "anchor": section["anchor"],
                "brief": section["brief"],
                "body": section["body"],
                "kind": section["kind"],
                "original": section["original"],
                "status": "drafted" if section["body"].strip() else "todo",
            }
        )
        order += 1

    intro = {
        "id": f"{slug(path)}__intro",
        "file": path,
        "fileId": slug(path),
        "area": area,
        "category": category,
        "order": -1,
        "heading": "",
        "anchor": "",
        "brief": [],
        "body": "",
        "kind": "intro",
        "original": "",
        "status": "todo",
    }
    if rows and rows[0].pop("_intro", False):
        intro = rows.pop(0)
    for row in rows:
        row.pop("_intro", None)
    rows.insert(0, intro)
    record["sectionCount"] = len(rows)
    return record, rows


def main():
    files, sections = [], []
    for order, path in enumerate(FILES):
        record, rows = parse(path)
        record["order"] = order
        files.append(record)
        sections.extend(rows)
    json.dump({"files": files, "sections": sections}, sys.stdout)
    print(f"{len(files)} files, {len(sections)} sections", file=sys.stderr)


main()
