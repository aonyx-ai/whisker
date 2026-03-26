extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_session;

/// Runs whisker as a rustc driver, registering all lint passes
///
/// This is invoked when cargo calls whisker as `RUSTC_WORKSPACE_WRAPPER`.
/// The binary acts as a rustc replacement that loads whisker's lints before
/// compiling the target crate.
///
/// Cargo passes the real rustc path as the first argument after the wrapper
/// binary name, so we skip `argv[0]` (the whisker path) and forward the rest
/// to [`rustc_driver::run_compiler`].
pub fn run() {
    rustc_driver::install_ice_hook("https://github.com/aonyx-ai/whisker/issues", |_| ());

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut callbacks = WhiskerCallbacks;
    rustc_driver::run_compiler(&args, &mut callbacks);
}

pub(crate) struct WhiskerCallbacks;

// r[impl driver.register-lints]
impl rustc_driver::Callbacks for WhiskerCallbacks {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        // r[impl driver.preserve-existing-lints]
        let previous = config.register_lints.take();
        config.register_lints = Some(Box::new(move |sess, store| {
            if let Some(previous) = &previous {
                previous(sess, store);
            }
            anyhow_missing_context::register_lints(sess, store);
            bool_param::register_lints(sess, store);
            derive_order::register_lints(sess, store);
            if_let_with_else::register_lints(sess, store);
            no_matches_macro::register_lints(sess, store);
            wildcard_match_arm::register_lints(sess, store);
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WhiskerCallbacks>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<WhiskerCallbacks>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<WhiskerCallbacks>();
    }
}
