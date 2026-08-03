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

r[cli.check.deny-warnings]
The `whisker check` command must accept a `--deny-warnings` flag that makes
warning diagnostics fail the run. Warnings must then be reported as errors
and must produce a non-zero exit code, while `help` and `info` diagnostics
must never fail the run.

r[cli.check.extra-args]
The `whisker check` command must forward trailing arguments to
`cargo check`.

r[cli.check.coverage]
The `whisker check` command must fail when a file it is asked to lint
cannot be decorated by any decoration provider, reporting the file, the
providers consulted, and each provider's reason for declining.

r[cli.version]
The `whisker --version` command must print the whisker version.

## File discovery

r[cli.discovery.ignore-files]
When the target path is a directory, `whisker check` must skip files
excluded by `.gitignore`, `.ignore`, `.git/info/exclude`, and the user's
global gitignore, as well as hidden files and directories. These files must
be honored whether or not the target is inside a git repository.

r[cli.discovery.explicit-target]
The target path itself must be checked even when an ignore rule would
exclude it, and the rule that excluded it must not be reapplied to what is
found inside it. Rules matching something further down still apply. A target
that names a file whisker has no grammar for is an error rather than a file
that is reported as clean without having been understood.

r[cli.discovery.walk-errors]
When a directory entry or an ignore file cannot be read, `whisker check`
must report the failure and continue if `--keep-going` is set, and must fail
otherwise.

r[cli.discovery.empty]
When discovery yields no files, `whisker check` must report that it analyzed
nothing and exit with a non-zero status rather than reporting success.

## Configuration

r[cli.config.workspace-metadata]
Whisker must read its configuration from the `[workspace.metadata.whisker]`
table of the target project's Cargo workspace manifest. A target outside a
Cargo workspace, or a workspace manifest without that table, uses the
default configuration.

r[cli.config.unknown-keys]
Whisker must reject a configuration table containing a key it does not
recognize rather than ignoring the key.

r[cli.config.ignore]
The configuration must accept an `ignore` key holding a list of
gitignore-syntax patterns whose matches are excluded from file discovery.
The patterns must behave as they would in a `.gitignore` written at the
workspace root: one containing an interior slash names a path relative to
that root, one without matches at any depth beneath it, and a leading slash
anchors a pattern that would otherwise match at any depth.

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
