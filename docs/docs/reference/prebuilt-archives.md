# Prebuilt archives

A repository of rules can publish its libraries already compiled. Whisker then
downloads them instead of compiling the source.

## The archive

| Part     | Value                                               |
| -------- | --------------------------------------------------- |
| Name     | `<rev>-<tag>.tar.gz`                                |
| `<rev>`  | The commit a project pins                           |
| `<tag>`  | What `whisker abi` prints on the consumer's machine |
| Digest   | A `.sha256` published beside the archive            |
| Contents | Regular files at the archive root, and nothing else |

The tag is a digest of every value the handshake compares, plus the target
triple. An archive published under a tag passes that Whisker's handshake.

Whisker unpacks only regular files at the root, which keeps every entry inside
the directory and passes over symbolic links.

## The lookup order

1. Libraries already unpacked in the cache.
2. A checkout already in the cache.
3. The remote's releases.
4. A fetch of the source, then a compile.

Whisker asks a release only when the cache holds nothing for the entry. See
[cache layout][cache] to force a fresh lookup.

## What it says

| Case                                                | Output                               |
| --------------------------------------------------- | ------------------------------------ |
| No release names an archive for this tag            | Nothing. Whisker compiles the source |
| The remote is not on GitHub, or the API answers 404 | Nothing                              |
| An API Whisker cannot reach, or an error answer     | One line on stderr                   |
| A digest that does not match                        | One line on stderr                   |
| An archive that will not unpack                     | One line on stderr                   |
| A cache directory Whisker cannot write              | One line on stderr                   |

Every case falls back to compiling the source. A prebuilt library that fails
the handshake is different: it ends the run and names the directory to delete,
because the publisher named the archive with a tag that does not describe it.

## Private repositories

Set `GH_TOKEN` or `GITHUB_TOKEN` to reach a private repository's releases, and
`WHISKER_GITHUB_API_URL` for GitHub Enterprise. See
[environment variables][environment-variables].

A repository Whisker cannot see looks the same as one that published nothing,
and stays quiet.

## What the digest proves

That the download arrived intact, and nothing else. See
[pinning and trust][pinning-and-trust].

[cache]: /docs/reference/cache-layout
[environment-variables]: /docs/reference/environment-variables
[pinning-and-trust]: /docs/explanation/pinning-and-trust
