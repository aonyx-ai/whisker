# Pin a shared set of rules

A repository of rules is how one set of lints reaches several projects. Name
the repository and the commit:

```toml title=".config/whisker.toml"
[[lints]]
git = "https://github.com/aonyx-ai/whisker-aonyx-rules"
rev = "0123456789abcdef0123456789abcdef01234567"
```

Whisker takes that one commit and nothing else. A branch or a tag is whatever
the remote points it at today, so the same configuration would run different
rules on different days.

## Find the commit to pin

`rev` is the full hash, 40 characters. An abbreviated one is refused:

```bash
git ls-remote https://github.com/aonyx-ai/whisker-aonyx-rules HEAD
```

## Move the pin

Change `rev` and run Whisker again. The new commit is fetched and built once
per machine, and the old checkout stays in the cache.

Moving a pin is when new rules reach the project, so expect new diagnostics.
[Adopt rules one at a time][choosing-rules] if there are more than the project
is ready for.

## What the first run costs

The first run on a machine fetches the commit and compiles it, which takes as
long as any Rust build. Later runs reuse the checkout, because a commit hash
names an immutable tree.

A repository that publishes [prebuilt archives][prebuilt-lints] skips the
compile. A machine that keeps its cache skips both; see
[cache layout][cache].

## If you publish the rules

A git entry builds with `--locked`, so the repository has to commit its
lockfile. Without one, every consumer's build fails.

[cache]: /docs/reference/cache-layout
[choosing-rules]: /docs/how-to/adopt-rules-gradually
[prebuilt-lints]: /docs/reference/prebuilt-archives
