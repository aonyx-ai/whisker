# Environment variables

<!--
goal: one table saying which variable changes what, and who reads it.
non-goal: explaining the cache or the token model. each has its own page.
-->

## the table

<!--
which variables exist. seven rows, each naming its reader, the install script or
whisker check.
where does my token go. only to the API host asked for releases.
why is my `GIT_*` environment ignored. the fetch uses the machine's git
credentials and ignores it so a run inside a hook cannot be redirected.
-->

| Variable                 | Read by            | Effect                                                                                          |
| ------------------------ | ------------------ | ----------------------------------------------------------------------------------------------- |
| `WHISKER_INSTALL_DIR`    | The install script | Where to put the binary. Defaults to `~/.local/bin`                                             |
| `WHISKER_CACHE_DIR`      | `whisker check`    | Where checkouts and prebuilt libraries live. Defaults to the XDG cache directory plus `whisker` |
| `XDG_CACHE_HOME`         | `whisker check`    | The cache root, when `WHISKER_CACHE_DIR` is unset                                               |
| `GH_TOKEN`               | `whisker check`    | Reaches a private repository's releases. Read first                                             |
| `GITHUB_TOKEN`           | `whisker check`    | The same, when `GH_TOKEN` is unset                                                              |
| `CARGO`                  | `whisker check`    | The cargo that builds a lint crate. Defaults to the one on `PATH`                               |
| `WHISKER_GITHUB_API_URL` | `whisker check`    | A GitHub Enterprise API to ask for prebuilt archives                                            |
