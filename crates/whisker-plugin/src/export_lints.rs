/// Exports lint passes as a custom lint plugin
///
/// Invoke this once, at the crate root of a `cdylib` lint crate. The lints
/// are grouped by the language their pass is written for, and each group
/// lists one expression per lint:
///
/// ```ignore
/// whisker_plugin::export_lints![rust: [NoTodo, PreferExpect]];
/// ```
///
/// A `rust` group takes types implementing [`RustLintPass`]. The group
/// decides the adapter that bridges a lint to [`LintPass`], and so the
/// language whisker runs it on. One type may appear in more than one
/// group: a rule that reads only comments can implement several language
/// traits and run on each. The rule names it declares are reported once.
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
/// own code under [`Panic::catch`]. A panic can happen in a constructor, in
/// the listing of the factories, or in a rule's declaration. It comes back
/// to whisker as a value rather than unwinding across the boundary.
///
/// [`LintPass`]: whisker_types::LintPass
/// [`Panic::catch`]: crate::Panic::catch
/// [`RustLintPass`]: whisker_rust::RustLintPass
#[macro_export]
macro_rules! export_lints {
    ($($language:ident: [$($lint:expr),+ $(,)?]),+ $(,)?) => {
        #[unsafe(no_mangle)]
        #[allow(non_upper_case_globals)]
        pub static whisker_plugin_declaration: $crate::PluginDeclaration =
            $crate::PluginDeclaration {
                abi_version: $crate::ABI_VERSION,
                types_fingerprint: $crate::TYPES_FINGERPRINT,
                language_fingerprint: $crate::LANGUAGE_FINGERPRINT,
                load: __whisker_load,
            };

        #[doc(hidden)]
        extern "C" fn __whisker_load() -> $crate::Loaded {
            $crate::Panic::catch(|| {
                let mut factories = $crate::Factories::new();
                let mut rules = ::std::vec::Vec::new();
                $($(
                    factories.push($crate::factory(|| {
                        $crate::Panic::catch(|| $crate::__adapter!($language)($lint)).into()
                    }));
                    for rule in $crate::DeclaresRules::rules(&$lint) {
                        if !rules.contains(&rule) {
                            rules.push(rule);
                        }
                    }
                )+)+
                $crate::Plugin {
                    factories,
                    rules: rules.into_iter().collect(),
                }
            })
            .into()
        }
    };
}

/// Names the function that boxes a lint of one language for the boundary
#[doc(hidden)]
#[macro_export]
macro_rules! __adapter {
    (rust) => {
        $crate::__private::whisker_rust::RustLintPassAdapter::boxed
    };
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use stabby::closure::Call0Dyn;
    use whisker_rust::RustLintPass;
    use whisker_types::{
        BoxedLintPass, DecoratedNode, DecoratedTree, Diagnostic, Language, LintPass, Panic, RuleId,
        Severity,
    };

    use crate::{ABI_VERSION, DeclaresRules, LANGUAGE_FINGERPRINT, Plugin, TYPES_FINGERPRINT};

    struct FlagEveryFunction;

    impl DeclaresRules for FlagEveryFunction {
        fn rules(&self) -> Vec<RuleId> {
            vec![RuleId::new("test.flag-every-function")]
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

    impl DeclaresRules for QuietLint {
        fn rules(&self) -> Vec<RuleId> {
            vec![RuleId::new("test.quiet")]
        }
    }

    impl RustLintPass for QuietLint {}

    export_lints![rust: [FlagEveryFunction, QuietLint, QuietLint]];

    fn loaded() -> Plugin {
        let loaded: Result<_, Panic> = (whisker_plugin_declaration.load)().into();

        loaded.expect("loading the plugin should not panic")
    }

    fn built(index: usize) -> BoxedLintPass {
        let constructed: Result<_, Panic> = loaded().factories[index].call().into();

        constructed.expect("construction should not panic")
    }

    #[test]
    fn declaration_carries_the_handshake_constants() {
        assert_eq!(whisker_plugin_declaration.abi_version, ABI_VERSION);

        let types = whisker_plugin_declaration.types_fingerprint;
        assert_eq!(types, TYPES_FINGERPRINT);

        let language = whisker_plugin_declaration.language_fingerprint;
        assert_eq!(language, LANGUAGE_FINGERPRINT);
    }

    #[test]
    fn factories_build_passes_of_their_group_language() {
        let pass = built(0);

        let language = pass.language();

        assert_eq!(language, Language::Rust);
    }

    #[test]
    fn factories_build_working_passes() {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&whisker_rust::language()).unwrap();
        let tree = parser.parse("fn main() {}", None).unwrap();
        let tree = DecoratedTree::new(
            tree,
            Language::Rust,
            "fn main() {}".into(),
            PathBuf::from("test.rs"),
        );
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
    fn load_yields_each_declared_rule_once_in_order() {
        let rules = loaded().rules;

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0], RuleId::new("test.flag-every-function"));
        assert_eq!(rules[1], RuleId::new("test.quiet"));
    }

    #[test]
    fn load_yields_one_factory_per_listed_lint() {
        let factories = loaded().factories;

        assert_eq!(factories.len(), 3);
    }
}
