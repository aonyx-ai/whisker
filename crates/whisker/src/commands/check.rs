use std::process::Command;

use anyhow::Context as _;
use clawless::prelude::*;

use crate::toolchain;

/// Run whisker lints against a Rust project
#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Path to the project's Cargo.toml
    #[arg(long)]
    manifest_path: Option<String>,

    /// Continue checking even if compilation fails for a package
    #[arg(long)]
    keep_going: bool,

    /// Additional arguments passed to cargo check
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

// r[impl cli.check]
#[command]
pub async fn check(args: CheckArgs, _context: Context) -> CommandResult {
    toolchain::ensure(toolchain::TOOLCHAIN)?;

    let self_path =
        std::env::current_exe().context("could not determine path to whisker binary")?;

    let mut cmd = Command::new("cargo");
    cmd.arg("check");

    // r[impl cli.check.manifest-path]
    if let Some(path) = &args.manifest_path {
        cmd.args(["--manifest-path", path]);
    }

    // r[impl cli.check.keep-going]
    if args.keep_going {
        cmd.arg("--keep-going");
    }

    // r[impl cli.check.extra-args]
    cmd.args(&args.args);

    cmd.env("RUSTC_WORKSPACE_WRAPPER", &self_path);
    cmd.env("RUSTUP_TOOLCHAIN", toolchain::TOOLCHAIN);
    cmd.env("__WHISKER_DRIVER", "1");

    let status = cmd.status().context("failed to run cargo check")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
