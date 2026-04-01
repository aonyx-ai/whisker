# whisker

## CLI

r[cli.check]
The `whisker check` command must run all whisker lints against the target
Rust project by invoking `cargo check` with the whisker binary as the
`RUSTC_WORKSPACE_WRAPPER`.

r[cli.check.manifest-path]
The `whisker check` command must accept a `--manifest-path` option to
specify the path to the target project's `Cargo.toml`.

r[cli.check.keep-going]
The `whisker check` command must accept a `--keep-going` flag that is
forwarded to `cargo check`.

r[cli.check.extra-args]
The `whisker check` command must forward trailing arguments to
`cargo check`.

r[cli.version]
The `whisker --version` command must print the whisker version.

## Toolchain management

r[cli.toolchain.auto-install]
The CLI must automatically install the required nightly toolchain via
`rustup toolchain install` if it is not already present, including the
`rustc-dev` and `llvm-tools-preview` components.

r[cli.toolchain.skip-installed]
The CLI must skip toolchain installation if the required toolchain is
already installed.

## Driver

r[driver.mode-detection]
The binary must detect whether it is running as a CLI or as a rustc driver
by checking for the `__WHISKER_DRIVER` environment variable.

r[driver.register-lints]
When running as a rustc driver, the binary must register all whisker lint
passes with the compiler's lint store via `rustc_driver::Callbacks`.

r[driver.preserve-existing-lints]
When registering lints, the driver must preserve any previously registered
lint callbacks (e.g., from rustc's built-in lints).
