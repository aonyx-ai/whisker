use std::process::Command;

use anyhow::{Context, Result, bail};

/// The nightly toolchain that this binary was compiled against.
/// Lint passes are compiled into the binary and require this exact toolchain.
pub const TOOLCHAIN: &str = "nightly-2026-03-05";

/// Ensures the required nightly toolchain is installed
///
/// Checks rustup's installed toolchains and installs the required nightly
/// with `rustc-dev` and `llvm-tools-preview` components if missing.
///
/// # Errors
///
/// Returns an error if rustup is not available or if the toolchain
/// installation fails.
// r[impl cli.toolchain.auto-install]
pub fn ensure(toolchain: &str) -> Result<()> {
    let output = Command::new("rustup")
        .args(["toolchain", "list"])
        .output()
        .context("failed to run rustup — is it installed?")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // r[impl cli.toolchain.skip-installed]
    if is_installed(toolchain, &stdout) {
        return Ok(());
    }

    eprintln!("installing required toolchain {toolchain}...");
    let status = Command::new("rustup")
        .args([
            "toolchain",
            "install",
            toolchain,
            "--component",
            "rustc-dev",
            "--component",
            "llvm-tools-preview",
        ])
        .status()
        .context("failed to install toolchain via rustup")?;

    if !status.success() {
        bail!("rustup toolchain install failed");
    }

    Ok(())
}

/// Checks whether a toolchain appears in `rustup toolchain list` output
///
/// Matches the toolchain name at the start of each line, allowing for
/// trailing target triples and status markers like `(default)`.
fn is_installed(toolchain: &str, rustup_output: &str) -> bool {
    rustup_output
        .lines()
        .any(|line| line.starts_with(toolchain))
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cli.toolchain.skip-installed]
    #[test]
    fn is_installed_with_exact_match() {
        let output =
            "stable-aarch64-apple-darwin (default)\nnightly-2026-03-05-aarch64-apple-darwin\n";

        assert!(is_installed("nightly-2026-03-05", output));
    }

    #[test]
    fn is_installed_with_default_marker() {
        let output =
            "nightly-2026-03-05-aarch64-apple-darwin (default)\nstable-aarch64-apple-darwin\n";

        assert!(is_installed("nightly-2026-03-05", output));
    }

    // r[verify cli.toolchain.auto-install]
    #[test]
    fn is_installed_with_no_match() {
        let output =
            "stable-aarch64-apple-darwin (default)\nnightly-2025-12-01-aarch64-apple-darwin\n";

        assert!(!is_installed("nightly-2026-03-05", output));
    }

    #[test]
    fn is_installed_with_empty_output() {
        assert!(!is_installed("nightly-2026-03-05", ""));
    }
}
