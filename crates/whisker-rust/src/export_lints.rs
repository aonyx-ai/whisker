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
/// crates this plugin was compiled against. Each lint is registered as a
/// factory, because passes are stateful and the check command constructs a
/// fresh set for every file. The factories are plain function pointers, so
/// each expression must construct its lint from nothing; an expression
/// that captures its surroundings does not compile.
///
/// [`RustLintPass`]: crate::RustLintPass
#[macro_export]
macro_rules! export_lints {
    ($($lint:expr),+ $(,)?) => {
        #[unsafe(no_mangle)]
        #[allow(non_upper_case_globals)]
        pub static whisker_plugin_declaration: $crate::plugin::PluginDeclaration =
            $crate::plugin::PluginDeclaration {
                abi_version: $crate::plugin::ABI_VERSION,
                rustc_version: $crate::plugin::RUSTC_VERSION.as_ptr(),
                types_fingerprint: $crate::plugin::TYPES_FINGERPRINT,
                language_fingerprint: $crate::plugin::LANGUAGE_FINGERPRINT,
                register: __whisker_register,
                rules: __whisker_rules,
            };

        #[doc(hidden)]
        fn __whisker_rules() -> ::std::vec::Vec<$crate::RuleId> {
            let mut rules = ::std::vec::Vec::new();
            $(
                rules.extend($crate::DeclaresRules::rules(&$lint));
            )+
            rules
        }

        #[doc(hidden)]
        fn __whisker_register(registrar: &mut dyn $crate::plugin::LintRegistrar) {
            $(
                registrar.register(|| {
                    ::std::boxed::Box::new($crate::RustLintPassAdapter::new($lint))
                });
            )+
        }
    };
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;
    use std::path::PathBuf;

    use whisker_types::plugin::LintRegistrar;
    use whisker_types::{DecoratedNode, DecoratedTree, Diagnostic, LintPass, RuleId, Severity};

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

    struct Collecting {
        factories: Vec<fn() -> Box<dyn LintPass>>,
    }

    impl LintRegistrar for Collecting {
        fn register(&mut self, factory: fn() -> Box<dyn LintPass>) {
            self.factories.push(factory);
        }
    }

    fn collected() -> Vec<fn() -> Box<dyn LintPass>> {
        let mut registrar = Collecting {
            factories: Vec::new(),
        };
        (whisker_plugin_declaration.register)(&mut registrar);
        registrar.factories
    }

    #[test]
    fn declaration_carries_the_handshake_constants() {
        assert_eq!(whisker_plugin_declaration.abi_version, plugin::ABI_VERSION);

        let rustc_version = unsafe { CStr::from_ptr(whisker_plugin_declaration.rustc_version) };
        assert_eq!(rustc_version, plugin::RUSTC_VERSION);

        let types = whisker_plugin_declaration.types_fingerprint;
        assert_eq!(types, plugin::TYPES_FINGERPRINT);

        let language = whisker_plugin_declaration.language_fingerprint;
        assert_eq!(language, plugin::LANGUAGE_FINGERPRINT);
    }

    #[test]
    fn register_yields_one_factory_per_lint() {
        assert_eq!(collected().len(), 2);
    }

    #[test]
    fn registered_factories_build_working_passes() {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&crate::language()).unwrap();
        let tree = parser.parse("fn main() {}", None).unwrap();
        let tree = DecoratedTree::new(tree, "fn main() {}".into(), PathBuf::from("test.rs"));
        let function = tree.root_node().named_child(0).expect("should parse a fn");

        let mut pass = collected()[0]();
        let diagnostics = pass.check_node(&function);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].rule_id(),
            RuleId::new("test.flag-every-function")
        );
    }
}
