use std::path::PathBuf;
use std::process::Command;

use anyhow::Context as _;
use clawless::prelude::*;

use crate::toolchain::Toolchain;

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

/// A configured `cargo check` invocation that uses whisker as the rustc driver
///
/// Encapsulates the arguments and environment needed to run `cargo check`
/// with whisker registered as `RUSTC_WORKSPACE_WRAPPER`. Separating command
/// construction from execution makes the argument-assembly logic testable
/// without spawning processes.
#[derive(Clone, Debug)]
pub(crate) struct CargoCheck {
    manifest_path: Option<String>,
    keep_going: bool,
    extra_args: Vec<String>,
    wrapper: PathBuf,
    toolchain: Toolchain,
}

impl CargoCheck {
    /// Builds the [`Command`] that runs `cargo check` with whisker as the driver
    pub(crate) fn command(&self) -> Command {
        let mut cmd = Command::new("cargo");
        cmd.arg("check");

        // r[impl cli.check.manifest-path]
        if let Some(path) = &self.manifest_path {
            cmd.args(["--manifest-path", path]);
        }

        // r[impl cli.check.keep-going]
        if self.keep_going {
            cmd.arg("--keep-going");
        }

        // r[impl cli.check.extra-args]
        cmd.args(&self.extra_args);

        cmd.env("RUSTC_WORKSPACE_WRAPPER", &self.wrapper);
        cmd.env("RUSTUP_TOOLCHAIN", self.toolchain.name());
        cmd.env("__WHISKER_DRIVER", "1");

        cmd
    }
}

// r[impl cli.check]
#[command]
pub async fn check(args: CheckArgs, _context: Context) -> CommandResult {
    Toolchain::REQUIRED.ensure()?;

    let CheckArgs {
        manifest_path,
        keep_going,
        args: extra_args,
    } = args;

    let wrapper =
        std::env::current_exe().context("could not determine path to whisker binary")?;

    let cargo_check = CargoCheck {
        manifest_path,
        keep_going,
        extra_args,
        wrapper,
        toolchain: Toolchain::REQUIRED,
    };

    let status = cargo_check
        .command()
        .status()
        .context("failed to run cargo check")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CargoCheck>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CargoCheck>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CargoCheck>();
    }

    fn base_cargo_check() -> CargoCheck {
        CargoCheck {
            manifest_path: None,
            keep_going: false,
            extra_args: Vec::new(),
            wrapper: PathBuf::from("/usr/bin/whisker"),
            toolchain: Toolchain::REQUIRED,
        }
    }

    fn args_of(cmd: &Command) -> Vec<&OsStr> {
        cmd.get_args().collect()
    }

    fn env_of<'a>(cmd: &'a Command, key: &str) -> Option<&'a OsStr> {
        cmd.get_envs()
            .find(|(k, _)| *k == key)
            .and_then(|(_, v)| v)
    }

    // r[verify cli.check]
    #[test]
    fn command_invokes_cargo_check() {
        let cmd = base_cargo_check().command();

        assert_eq!(cmd.get_program(), "cargo");
        assert_eq!(args_of(&cmd)[0], "check");
    }

    #[test]
    fn command_sets_driver_env() {
        let cmd = base_cargo_check().command();

        assert_eq!(env_of(&cmd, "RUSTC_WORKSPACE_WRAPPER").unwrap(), "/usr/bin/whisker");
        assert_eq!(
            env_of(&cmd, "RUSTUP_TOOLCHAIN").unwrap(),
            Toolchain::REQUIRED.name()
        );
        assert_eq!(env_of(&cmd, "__WHISKER_DRIVER").unwrap(), "1");
    }

    // r[verify cli.check.manifest-path]
    #[test]
    fn command_with_manifest_path_includes_flag() {
        let cargo_check = CargoCheck {
            manifest_path: Some("/tmp/Cargo.toml".into()),
            ..base_cargo_check()
        };

        let cmd = cargo_check.command();
        let args = args_of(&cmd);

        assert_eq!(args[1], "--manifest-path");
        assert_eq!(args[2], "/tmp/Cargo.toml");
    }

    #[test]
    fn command_without_manifest_path_omits_flag() {
        let cmd = base_cargo_check().command();
        let args = args_of(&cmd);

        assert!(!args.contains(&OsStr::new("--manifest-path")));
    }

    // r[verify cli.check.keep-going]
    #[test]
    fn command_with_keep_going_includes_flag() {
        let cargo_check = CargoCheck {
            keep_going: true,
            ..base_cargo_check()
        };

        let cmd = cargo_check.command();
        let args = args_of(&cmd);

        assert!(args.contains(&OsStr::new("--keep-going")));
    }

    #[test]
    fn command_without_keep_going_omits_flag() {
        let cmd = base_cargo_check().command();
        let args = args_of(&cmd);

        assert!(!args.contains(&OsStr::new("--keep-going")));
    }

    // r[verify cli.check.extra-args]
    #[test]
    fn command_with_extra_args_appends_them() {
        let cargo_check = CargoCheck {
            extra_args: vec!["-p".into(), "my_crate".into()],
            ..base_cargo_check()
        };

        let cmd = cargo_check.command();
        let args = args_of(&cmd);

        assert!(args.contains(&OsStr::new("-p")));
        assert!(args.contains(&OsStr::new("my_crate")));
    }
}
