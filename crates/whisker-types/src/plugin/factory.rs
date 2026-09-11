use stabby::{result, vec};

use crate::{BoxedLintPass, Panic, RuleId};

/// Constructs one fresh lint pass
///
/// A plugin hands over a constructor for each lint. Passes are stateful,
/// and the check command constructs a fresh set for every file.
///
/// The factory is one of stabby's boxed callables, so a plugin builds it
/// from a plain closure and the host calls it through a vtable stabby
/// lays out. Construction runs plugin code, so a panic in it comes back
/// as a value like any other.
pub type LintPassFactory =
    stabby::dynptr!(stabby::boxed::Box<dyn Fn() -> Constructed + Send + Sync>);

/// What a [`LintPassFactory`] hands back: a pass, or the panic that stopped it
pub type Constructed = result::Result<BoxedLintPass, Panic>;

/// Every factory a plugin exports, in the order it named its lints
///
/// The list is stabby's, because the plugin allocates it and whisker
/// frees it.
pub type Factories = vec::Vec<LintPassFactory>;

/// Returns the factory that runs `build`
///
/// A factory is one of stabby's boxed callables, and building one means
/// naming a stabby type. This keeps that name in this crate, so a plugin
/// exports lints without depending on stabby itself.
pub fn factory(build: impl Fn() -> Constructed + Send + Sync + 'static) -> LintPassFactory {
    stabby::boxed::Box::new(build).into()
}

/// Runs `factory` and returns the pass it built
///
/// The factory is a stabby callable, and calling one means naming a
/// stabby trait. This keeps that name in this crate, so the loader
/// constructs a pass without depending on stabby itself.
pub fn construct(factory: &LintPassFactory) -> Constructed {
    use stabby::closure::Call0Dyn;

    factory.call()
}

/// Everything a plugin exports, handed over in one call
///
/// The factories and the rule list travel together, because building
/// either runs plugin code and one call is the whole boundary. A plugin
/// that gains something else to declare puts it here rather than on a
/// second function.
#[stabby::stabby]
pub struct Plugin {
    /// A factory for every lint the plugin exports
    pub factories: Factories,

    /// Every rule the plugin can report
    ///
    /// A project names the rules it runs, and whisker refuses a name that
    /// no loaded plugin declares. A misspelled name would otherwise
    /// disable nothing and report nothing, which reads like a rule that
    /// found no fault.
    pub rules: vec::Vec<RuleId>,
}

/// What [`PluginDeclaration::load`] hands back: the plugin, or the panic that stopped it
///
/// Building either list runs plugin code, so it carries a panic like
/// every other call into the plugin.
///
/// [`PluginDeclaration::load`]: crate::plugin::PluginDeclaration::load
pub type Loaded = result::Result<Plugin, Panic>;

#[cfg(test)]
mod tests {
    use stabby::closure::Call0Dyn;
    use stabby::vec;

    use super::*;
    use crate::{Checked, Configured, DecoratedNode, LintPass, RuleOptions};

    struct Quiet;

    impl LintPass for Quiet {
        extern "C" fn configure(&mut self, _options: &RuleOptions) -> Configured {
            Configured::Ok(())
        }

        extern "C" fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Checked {
            Checked::Ok(vec::Vec::new())
        }
    }

    fn quiet() -> LintPassFactory {
        stabby::boxed::Box::new(|| Constructed::Ok(crate::boxed_lint_pass(Quiet))).into()
    }

    #[test]
    fn a_factory_builds_a_pass_the_host_can_configure() {
        let mut factories = Factories::new();
        factories.push(quiet());

        let built: Result<_, Panic> = factories[0].call().into();

        let mut pass = built.expect("construction should not panic");
        let configured: Result<(), Panic> = pass.configure(&RuleOptions::default()).into();
        assert!(configured.is_ok());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LintPassFactory>();
        assert_send::<Factories>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LintPassFactory>();
        assert_sync::<Factories>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LintPassFactory>();
        assert_unpin::<Factories>();
    }
}
