use std::process::Command;

use anyhow::{Context, Result, bail};

/// A pinned nightly Rust toolchain required by the whisker driver
///
/// Whisker lint passes are compiled against a specific nightly rustc and
/// require that exact toolchain at runtime. This type encapsulates the
/// toolchain name and handles installation via rustup.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Toolchain(&'static str);

impl Toolchain {
    /// The nightly toolchain this binary was compiled against
    pub const REQUIRED: Self = Self("nightly-2026-03-05");

    /// Returns the toolchain name
    pub fn name(&self) -> &str {
        self.0
    }

    /// Ensures this toolchain is installed via rustup
    ///
    /// Checks rustup's installed toolchains and installs the toolchain
    /// with `rustc-dev` and `llvm-tools-preview` components if missing.
    ///
    /// # Errors
    ///
    /// Returns an error if rustup is not available or if the toolchain
    /// installation fails.
    // r[impl cli.toolchain.auto-install]
    pub fn ensure(&self) -> Result<()> {
        let output = Command::new("rustup")
            .args(["toolchain", "list"])
            .output()
            .context("failed to run rustup — is it installed?")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // r[impl cli.toolchain.skip-installed]
        if self.is_installed(&stdout) {
            return Ok(());
        }

        eprintln!("installing required toolchain {}...", self.0);
        let status = Command::new("rustup")
            .args([
                "toolchain",
                "install",
                self.0,
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

    /// Checks whether this toolchain appears in `rustup toolchain list` output
    ///
    /// Matches the toolchain name at the start of each line, allowing for
    /// trailing target triples and status markers like `(default)`.
    fn is_installed(&self, rustup_output: &str) -> bool {
        rustup_output
            .lines()
            .any(|line| line.starts_with(self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Toolchain>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Toolchain>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Toolchain>();
    }

    // r[verify cli.toolchain.skip-installed]
    #[test]
    fn is_installed_with_exact_match() {
        let toolchain = Toolchain::REQUIRED;
        let output =
            "stable-aarch64-apple-darwin (default)\nnightly-2026-03-05-aarch64-apple-darwin\n";

        assert!(toolchain.is_installed(output));
    }

    #[test]
    fn is_installed_with_default_marker() {
        let toolchain = Toolchain::REQUIRED;
        let output =
            "nightly-2026-03-05-aarch64-apple-darwin (default)\nstable-aarch64-apple-darwin\n";

        assert!(toolchain.is_installed(output));
    }

    // r[verify cli.toolchain.auto-install]
    #[test]
    fn is_installed_with_no_match() {
        let toolchain = Toolchain::REQUIRED;
        let output =
            "stable-aarch64-apple-darwin (default)\nnightly-2025-12-01-aarch64-apple-darwin\n";

        assert!(!toolchain.is_installed(output));
    }

    #[test]
    fn is_installed_with_empty_output() {
        let toolchain = Toolchain::REQUIRED;

        assert!(!toolchain.is_installed(""));
    }
}
