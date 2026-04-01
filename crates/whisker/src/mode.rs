/// Execution mode of the whisker binary
///
/// Whisker serves dual roles: a user-facing CLI tool and a rustc driver
/// that cargo invokes as `RUSTC_WORKSPACE_WRAPPER`. The mode is determined
/// at startup by the presence of an environment variable.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Mode {
    /// Running as a rustc driver, invoked by cargo via `RUSTC_WORKSPACE_WRAPPER`
    Driver,

    /// Running as a user-facing CLI tool
    Cli,
}

impl Mode {
    /// Detects the execution mode from the process environment
    ///
    /// Returns [`Mode::Driver`] when the `__WHISKER_DRIVER` environment
    /// variable is set, [`Mode::Cli`] otherwise.
    pub fn detect() -> Self {
        match std::env::var("__WHISKER_DRIVER") {
            Ok(_) => Self::Driver,
            Err(_) => Self::Cli,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Mode>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Mode>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Mode>();
    }
}
