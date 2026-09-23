# Whisker

Whisker is a linting platform built on [tree-sitter][ts]. It lints Rust
today. Whisker ships no rules of its own: a project configures the ones it
wants, and those are exactly the ones it runs. Aonyx's own live in
[whisker-aonyx-rules][rules] and cover what Clippy does not, like derive
ordering, wildcard match arms, `matches!` macro usage, and the other style
rules defined in our `CLAUDE.md` files.

A rule implements the `RustLintPass` trait, which whisker generates from
tree-sitter's Rust grammar. A rule that needs type information reads
decorations that whisker computes with [rust-analyzer][ra] before any rule
runs.

## Status

Whisker is in early development. Check back soon.

## Quick start

Download the archive for your platform from the [releases page][releases],
check it against the `.sha256` beside it, and unpack it:

```bash
shasum -a 256 -c whisker-0.1.0-rc.4-aarch64-apple-darwin.tar.gz.sha256
tar -xzf whisker-0.1.0-rc.4-aarch64-apple-darwin.tar.gz
```

The archive unpacks to a directory that holds the binary, both licenses, and
this README. Move `whisker` to a directory on your `PATH`.

Whisker runs no rules until a project names some. Write
`.config/whisker.toml` at the top of your repository and pin a source of
rules to one commit:

```toml
[[lints]]
git = "https://github.com/aonyx-ai/whisker-aonyx-rules"
rev = "ffd02b34a39900a84045ff7bc0130885bf9f5732"
```

Then check the project:

```bash
whisker check .
```

The first run builds the rules, which takes as long as any Rust compilation.
Later runs reuse cargo's cache.

## Documentation

The [documentation][docs] covers the rest:

- [Install Whisker][install] — release archives, the GitHub Actions action,
  and building from source.
- [Runs and outcomes][checking] — what ends a run, and what fails it.
- [Configuration][configuration] — every key of `.config/whisker.toml`.
- [Write a rule][custom-lints] — hook a node kind, test it, and give it
  options.
- [Prebuilt archives][prebuilt-lints] — publish compiled rules so a project
  skips the build.

[ARCHITECTURE.md](ARCHITECTURE.md) describes how Whisker works inside.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)
  or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT)
  or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[checking]: https://aonyx-ai.github.io/whisker/docs/reference/runs-and-outcomes
[configuration]: https://aonyx-ai.github.io/whisker/docs/reference/configuration
[custom-lints]: https://aonyx-ai.github.io/whisker/authoring/how-to/write-a-rule
[docs]: https://aonyx-ai.github.io/whisker/
[install]: https://aonyx-ai.github.io/whisker/docs/how-to/install
[prebuilt-lints]: https://aonyx-ai.github.io/whisker/docs/reference/prebuilt-archives
[ra]: https://rust-analyzer.github.io/
[releases]: https://github.com/aonyx-ai/whisker/releases
[rules]: https://github.com/aonyx-ai/whisker-aonyx-rules
[ts]: https://tree-sitter.github.io/tree-sitter/
