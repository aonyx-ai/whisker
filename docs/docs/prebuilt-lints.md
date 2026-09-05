---
sidebar_position: 5
---

# Prebuilt lints

A repository can publish its rules already compiled. Before whisker fetches a
git entry, it asks that repository's releases for an archive. The archive is
named after the pinned commit and after the tag that `whisker abi` prints. If a
release carries that archive and the `.sha256` beside it, whisker downloads it
and checks the digest. It then unpacks the archive into the cache and loads the
libraries. Each library still completes the handshake described in
[custom lints](/docs/custom-lints#the-plugin-boundary), and one that fails it
ends the run.

## When whisker asks

Whisker asks a release only when the cache holds nothing for the entry.
Libraries it unpacked before come first, then a checkout it compiled before. A
project with a cached checkout therefore keeps compiling it after its rules
start to publish archives. Move the pin or clear the cache to pick the archives
up.

A repository that publishes nothing for your tag is the ordinary case, and
whisker compiles the source and says nothing. A download that fails or a digest
that does not match prints one line on stderr, and whisker compiles the source.

## Private and enterprise repositories

Set `GH_TOKEN` or `GITHUB_TOKEN` to reach a private repository, and
`WHISKER_GITHUB_API_URL` to point whisker at a GitHub Enterprise installation.
Whisker sends the token only to that API.

## What the digest proves

The digest proves that the download arrived intact. The same publisher writes
the archive and the digest, so the digest establishes no trust in the
publisher. A repository you configure runs its code in whisker's process,
prebuilt or compiled.
