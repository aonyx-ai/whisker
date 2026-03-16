# Whisker

Whisker is a collection of custom [Dylint][dylint] lints that enforce Aonyx's
Rust coding conventions. These lints catch patterns that Clippy doesn't cover,
like derive ordering, wildcard match arms, `matches!` macro usage, and other
style rules defined in our `CLAUDE.md` files.

Each lint is a separate `cdylib` crate in the `lints/` directory.

## Status

Whisker is in early development. Check back soon.

## Usage

Install Dylint and the Whisker lints:

```bash
cargo install cargo-dylint dylint-link
```

Add Whisker to your workspace `Cargo.toml`:

```toml
[workspace.metadata.dylint]
libraries = [
    { git = "https://github.com/aonyx-ai/whisker", pattern = "lints/*" },
]
```

Run the lints:

```bash
cargo dylint --all
```

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

[dylint]: https://github.com/trailofbits/dylint
