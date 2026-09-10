# Match the toolchain to a released Whisker

A released Whisker loads only a library that its own rustc compiled. A project
that compiles its own rules therefore needs that same toolchain, whether the
rules come from a directory or from a repository.

You do not need this page when the rules you use publish
[prebuilt archives][prebuilt-lints], because then nothing is compiled.

## The error this fixes

```text
plugin built by `rustc 1.94.0 (...)`, whisker by `rustc 1.95.0-nightly (...)`; use one toolchain
```

The message names both compilers. The one it calls `whisker` is the toolchain
you have to match.

## Pin the toolchain beside the rules

Whisker runs `cargo build --release` inside the lint crate's directory, so
rustup reads the `rust-toolchain.toml` nearest that directory. Write one there,
naming the toolchain the release was built with:

```toml title="lints/doc_summary_break/rust-toolchain.toml"
[toolchain]
channel = "nightly-2026-09-02"
```

The release's own `rust-toolchain.toml` is the source of that value. Read it at
the tag you installed, such as [`v0.1.0-rc.4`][toolchain-file].

Nightly is not a requirement of the rule you write. Whisker's own toolchain file
pins nightly for an unstable rustfmt option, and a lint crate needs no unstable
feature. The pin exists only so both images come from one compiler.

## Pin the Whisker dependencies to the same release

The compiler is one of the values the handshake compares, and the layout of
Whisker's own types is another. Depend on the tag you installed:

```toml title="lints/doc_summary_break/Cargo.toml"
[dependencies]
whisker-rust = { git = "https://github.com/aonyx-ai/whisker.git", tag = "v0.1.0-rc.4", default-features = false }
whisker-types = { git = "https://github.com/aonyx-ai/whisker.git", tag = "v0.1.0-rc.4" }
```

A mismatch here reports itself separately, as a plugin built against another
whisker-types or another whisker-rust.

## The other way around

Building Whisker from source instead lets one `rust-toolchain.toml` govern
both. See [install Whisker][installation].

Whichever way you go, both images have to come from one compiler.
[The plugin boundary][plugin-boundary] explains why the check cannot be
loosened.

[installation]: /docs/how-to/install
[toolchain-file]: https://github.com/aonyx-ai/whisker/blob/v0.1.0-rc.4/rust-toolchain.toml
[plugin-boundary]: /authoring/explanation/plugin-boundary
[prebuilt-lints]: /docs/reference/prebuilt-archives
