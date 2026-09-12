# Run Whisker on GitHub Actions

The action in the Whisker repository downloads a release, checks it against the
digest published beside it, and puts `whisker` on the runner's `PATH`:

```yaml
- uses: aonyx-ai/whisker@v0.1.0-rc.4
  with:
    version: v0.1.0-rc.4
- run: whisker check .
```

## Rules the runner has to compile

A runner starts cold on every run, so a compile hurts most here.

Prefer rules that publish [prebuilt archives][prebuilt-lints], which skips the
build. When they publish nothing, the runner compiles them, so the job needs a
Rust toolchain and pays for the build on every run. Any toolchain will do,
because Whisker loads a plugin whatever rustc built it.

## Private repositories of rules

Whisker reads `GH_TOKEN`, then `GITHUB_TOKEN`, when it asks a repository's
releases for prebuilt archives, and sends the token only to that API host:

```yaml
- run: whisker check .
  env:
    GH_TOKEN: ${{ secrets.RULES_TOKEN }}
```

The runner's own `GITHUB_TOKEN` reaches only the repository the workflow runs
in. Rules elsewhere need a token that reaches that repository.

Fetching the source is separate: that fetch uses the machine's git credentials
rather than this token. See [environment variables][environment-variables].

[environment-variables]: /docs/reference/environment-variables
[prebuilt-lints]: /docs/reference/prebuilt-archives
