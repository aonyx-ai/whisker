# Environment variables

| Variable                 | Read by            | Effect                                                                                          |
| ------------------------ | ------------------ | ----------------------------------------------------------------------------------------------- |
| `WHISKER_INSTALL_DIR`    | The install script | Where to put the binary. Defaults to `~/.local/bin`                                             |
| `WHISKER_CACHE_DIR`      | `whisker check`    | Where checkouts and prebuilt libraries live. Defaults to the XDG cache directory plus `whisker` |
| `XDG_CACHE_HOME`         | `whisker check`    | The cache root, when `WHISKER_CACHE_DIR` is unset                                               |
| `GH_TOKEN`               | `whisker check`    | Reaches a private repository's releases. Read first                                             |
| `GITHUB_TOKEN`           | `whisker check`    | The same, when `GH_TOKEN` is unset                                                              |
| `CARGO`                  | `whisker check`    | The cargo that builds a lint crate. Defaults to the one on `PATH`                               |
| `WHISKER_GITHUB_API_URL` | `whisker check`    | A GitHub Enterprise API to ask for prebuilt archives                                            |

Whisker sends a token only to the API host it asks for releases.

Fetching the source of a repository is separate: that fetch carries the
machine's own git credentials, and ignores the `GIT_*` environment so that a
run inside a git hook cannot be redirected.

[Cache layout][cache] covers what the cache directory holds.

[cache]: /docs/reference/cache-layout
