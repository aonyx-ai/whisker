use crate::LintPass;

/// Collects the lint passes a custom lint plugin exports
///
/// The host hands an implementation to [`PluginDeclaration::register`],
/// and the plugin calls [`register`] once per lint. A plugin registers a
/// factory rather than a pass, because passes are stateful and the check
/// command constructs a fresh set for every file; the CLI's pass list
/// gives the built-in lints that same per-file construction. A plain
/// function pointer suffices, because a lint is a unit struct
/// the factory constructs from nothing, and it keeps captured state out of
/// the plugin boundary.
///
/// [`PluginDeclaration::register`]: crate::plugin::PluginDeclaration::register
/// [`register`]: LintRegistrar::register
pub trait LintRegistrar {
    /// Registers one lint pass factory
    fn register(&mut self, factory: LintPassFactory);
}

/// Constructs one fresh lint pass
///
/// See [`LintRegistrar`] for why plugins hand over construction rather
/// than passes.
pub type LintPassFactory = fn() -> Box<dyn LintPass>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecoratedNode, Diagnostic};

    struct Collecting {
        factories: Vec<fn() -> Box<dyn LintPass>>,
    }

    impl LintRegistrar for Collecting {
        fn register(&mut self, factory: fn() -> Box<dyn LintPass>) {
            self.factories.push(factory);
        }
    }

    struct Quiet;

    impl LintPass for Quiet {
        fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            Vec::new()
        }
    }

    #[test]
    fn register_collects_working_factories() {
        let mut registrar = Collecting {
            factories: Vec::new(),
        };

        registrar.register(|| Box::new(Quiet));

        assert_eq!(registrar.factories.len(), 1);
        let _pass = registrar.factories[0]();
    }
}
