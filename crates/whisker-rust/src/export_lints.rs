/// Exports Rust lint passes as a custom lint plugin
///
/// Invoke this once, at the crate root of a `cdylib` lint crate, with one
/// expression per lint. Each expression must construct a type implementing
/// [`RustLintPass`]:
///
/// ```ignore
/// whisker_rust::export_lints![NoTodo, PreferExpect];
/// ```
///
/// The macro writes the `whisker_plugin_declaration` static that whisker's
/// loader looks up, filling in the handshake constants of the whisker
/// crates this plugin was compiled against. Each lint becomes a factory,
/// because passes are stateful and the check command constructs a fresh
/// set for every file. The factories are plain function pointers, so each
/// expression must construct its lint from nothing; an expression that
/// captures its surroundings does not compile.
///
/// Every function the macro exports is `extern "C"` and runs the plugin's
/// own code under [`Panic::catch`]. A panic in a constructor, in the
/// listing of the factories, or in a rule's declaration therefore comes
/// back to whisker as a value rather than unwinding across the boundary.
///
/// [`Panic::catch`]: crate::plugin::Panic::catch
/// [`RustLintPass`]: crate::RustLintPass
#[macro_export]
macro_rules! export_lints {
    ($($lint:expr),+ $(,)?) => {
        #[unsafe(no_mangle)]
        #[allow(non_upper_case_globals)]
        pub static whisker_plugin_declaration: $crate::plugin::PluginDeclaration =
            $crate::plugin::PluginDeclaration {
                abi_version: $crate::plugin::ABI_VERSION,
                types_fingerprint: $crate::plugin::TYPES_FINGERPRINT,
                language_fingerprint: $crate::plugin::LANGUAGE_FINGERPRINT,
                load: __whisker_load,
            };

        #[doc(hidden)]
        extern "C" fn __whisker_load() -> $crate::plugin::Loaded {
            $crate::plugin::Panic::catch(|| {
                let mut factories = $crate::plugin::Factories::new();
                let mut rules = ::std::vec::Vec::new();
                $(
                    factories.push($crate::plugin::factory(|| {
                        $crate::plugin::Panic::catch(|| {
                            $crate::RustLintPassAdapter::boxed($lint)
                        })
                        .into()
                    }));
                    rules.extend($crate::DeclaresRules::rules(&$lint));
                )+
                $crate::plugin::Plugin {
                    factories,
                    rules: rules.into_iter().collect(),
                }
            })
            .into()
        }
    };
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use stabby::closure::Call0Dyn;
    use whisker_types::{
        BoxedLintPass, DecoratedNode, DecoratedTree, Diagnostic, LintPass, Panic, RuleId, Severity,
    };

    use crate::RustLintPass;
    use crate::plugin;

    struct FlagEveryFunction;

    impl crate::DeclaresRules for FlagEveryFunction {
        fn rules(&self) -> Vec<whisker_types::RuleId> {
            vec![whisker_types::RuleId::new("test.flag-every-function")]
        }
    }

    impl RustLintPass for FlagEveryFunction {
        fn check_function_item(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            vec![Diagnostic::new(
                RuleId::new("test.flag-every-function"),
                Severity::Warn,
                "found a function".into(),
                node.span(),
            )]
        }
    }

    struct QuietLint;

    impl crate::DeclaresRules for QuietLint {
        fn rules(&self) -> Vec<whisker_types::RuleId> {
            vec![whisker_types::RuleId::new("test.quiet")]
        }
    }

    impl RustLintPass for QuietLint {}

    export_lints![FlagEveryFunction, QuietLint];

    fn loaded() -> plugin::Plugin {
        let loaded: Result<_, Panic> = (whisker_plugin_declaration.load)().into();

        loaded.expect("loading the plugin should not panic")
    }

    fn built(index: usize) -> BoxedLintPass {
        let constructed: Result<_, Panic> = loaded().factories[index].call().into();

        constructed.expect("construction should not panic")
    }

    #[test]
    fn declaration_carries_the_handshake_constants() {
        assert_eq!(whisker_plugin_declaration.abi_version, plugin::ABI_VERSION);

        let types = whisker_plugin_declaration.types_fingerprint;
        assert_eq!(types, plugin::TYPES_FINGERPRINT);

        let language = whisker_plugin_declaration.language_fingerprint;
        assert_eq!(language, plugin::LANGUAGE_FINGERPRINT);
    }

    #[test]
    fn factories_build_working_passes() {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&crate::language()).unwrap();
        let tree = parser.parse("fn main() {}", None).unwrap();
        let tree = DecoratedTree::new(tree, "fn main() {}".into(), PathBuf::from("test.rs"));
        let function = tree.root_node().named_child(0).expect("should parse a fn");
        let mut pass = built(0);

        let checked: Result<_, Panic> = LintPass::check_node(&mut pass, &function).into();

        let diagnostics = checked.expect("the pass should not panic");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].rule_id(),
            RuleId::new("test.flag-every-function")
        );
    }

    #[test]
    fn load_yields_one_factory_per_lint() {
        let factories = loaded().factories;

        assert_eq!(factories.len(), 2);
    }

    #[test]
    fn load_yields_every_declared_rule_in_order() {
        let rules = loaded().rules;

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0], RuleId::new("test.flag-every-function"));
        assert_eq!(rules[1], RuleId::new("test.quiet"));
    }
}
