use std::path::Path;

use anyhow::Context as _;
use whisker_types::{
    Coverage, DecoratedTree, DecorationProvider, Diagnostic, Language, LintPass, UncoveredFile,
};

use crate::tree_walker;

/// Orchestrates the parse-decorate-execute pipeline
///
/// The pipeline reads a source file, parses it with the appropriate
/// tree-sitter grammar, runs decoration providers, then walks the
/// decorated tree through all enabled lint passes.
pub struct Pipeline {
    parser: tree_sitter::Parser,
}

impl Pipeline {
    /// Creates a pipeline configured for the given language
    ///
    /// # Errors
    ///
    /// Returns an error if the tree-sitter language cannot be set.
    pub fn new(language: &tree_sitter::Language) -> anyhow::Result<Self> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(language)
            .context("failed to set tree-sitter language")?;
        Ok(Self { parser })
    }

    /// Runs the full pipeline on a source file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read. Any other error comes
    /// from [`Pipeline::run_on_source`].
    pub fn run(
        &mut self,
        path: &Path,
        providers: &[&dyn DecorationProvider],
        passes: &mut [Box<dyn LintPass>],
    ) -> anyhow::Result<Vec<Diagnostic>> {
        let source =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;

        self.run_on_source(&source, path, providers, passes)
    }

    /// Runs the pipeline on source text directly
    ///
    /// The pipeline offers the file to every provider, and one claim is
    /// enough. If no provider claims the file, the pipeline returns an
    /// [`UncoveredFile`] error instead of a false clean result.
    ///
    /// Decorations merge in provider order. The first decoration of a
    /// type on a node is the one lint rules see.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails, if a provider's toolchain
    /// malfunctions, or if no provider claims the file. An empty provider
    /// list counts as no claims.
    pub fn run_on_source(
        &mut self,
        source: &str,
        path: &Path,
        providers: &[&dyn DecorationProvider],
        passes: &mut [Box<dyn LintPass>],
    ) -> anyhow::Result<Vec<Diagnostic>> {
        let tree = self
            .parser
            .parse(source, None)
            .context("tree-sitter parse failed")?;

        let mut decorated = DecoratedTree::new(tree, source.to_string(), path.to_path_buf());

        let mut covered = Vec::new();
        let mut gaps = Vec::new();

        for provider in providers {
            let coverage = provider
                .decorate(&decorated)
                .with_context(|| format!("run the `{}` decoration provider", provider.name()))?;

            match coverage {
                Coverage::Covered(decorations) => covered.push(decorations),
                Coverage::NotCovered(gap) => gaps.push((provider.name(), gap)),
            }
        }

        if covered.is_empty() {
            return Err(UncoveredFile::new(path.to_path_buf(), gaps).into());
        }

        for decorations in covered {
            decorated.merge_decorations(decorations);
        }

        Ok(tree_walker::walk(&decorated, passes))
    }
}

/// Detects the language of a source file from its extension
///
/// # Errors
///
/// Returns an error if the extension is missing or unrecognized.
pub fn detect_language(path: &Path) -> anyhow::Result<Language> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .context("file has no extension")?;

    Language::from_extension(ext)
        .with_context(|| format!("unsupported language for extension `.{ext}`"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{
        CoverageGap, DecoratedNode, Decoration, DecorationKey, DecorationMap, Diagnostic, Language,
        ProviderName, RuleId, Severity,
    };

    use super::*;

    /// Claims every file and decorates nothing
    struct Covering;

    impl DecorationProvider for Covering {
        fn name(&self) -> ProviderName {
            ProviderName("covering")
        }

        fn decorate(&self, _tree: &DecoratedTree) -> anyhow::Result<Coverage> {
            Ok(Coverage::Covered(DecorationMap::new()))
        }
    }

    /// Claims nothing, so the pipeline must treat the file as unanalyzable
    struct Declining;

    impl DecorationProvider for Declining {
        fn name(&self) -> ProviderName {
            ProviderName("declining")
        }

        fn decorate(&self, _tree: &DecoratedTree) -> anyhow::Result<Coverage> {
            Ok(Coverage::NotCovered(CoverageGap::StaleSource))
        }
    }

    /// Claims every file and puts a known marker on the root node
    struct Decorating(&'static str);

    impl DecorationProvider for Decorating {
        fn name(&self) -> ProviderName {
            ProviderName("decorating")
        }

        fn decorate(&self, tree: &DecoratedTree) -> anyhow::Result<Coverage> {
            let mut decorations = DecorationMap::new();
            decorations.insert(tree.root_node().id(), Marker(self.0));
            Ok(Coverage::Covered(decorations))
        }
    }

    #[derive(Debug)]
    struct Marker(&'static str);

    unsafe impl Decoration for Marker {
        const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Marker"));

        type Ref<'a> = Option<&'a Self>;

        fn lookup<'a>(node: &DecoratedNode<'a>) -> Self::Ref<'a> {
            node.decoration::<Self>()
        }
    }

    /// Reports whichever [`Marker`] a provider left on the root node
    ///
    /// The assertion runs through a lint pass, so the test observes what
    /// a real rule would see, not what the pipeline happened to store.
    struct ReportMarker;

    impl LintPass for ReportMarker {
        fn check_node(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
            if node.kind() != "source_file" {
                return Vec::new();
            }
            let Some(marker) = node.decoration::<Marker>() else {
                return Vec::new();
            };
            vec![Diagnostic::new(
                RuleId::new("test.marker"),
                Severity::Warn,
                marker.0.into(),
                node.span(),
            )]
        }
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Pipeline>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Pipeline>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Pipeline>();
    }

    #[test]
    fn detect_language_with_rs_returns_rust() {
        let lang = detect_language(Path::new("test.rs")).expect("should detect");
        assert_eq!(lang, Language::Rust);
    }

    #[test]
    fn detect_language_with_unknown_extension_returns_error() {
        let result = detect_language(Path::new("test.py"));
        assert!(result.is_err());
    }

    #[test]
    fn detect_language_with_no_extension_returns_error() {
        let result = detect_language(Path::new("Makefile"));
        assert!(result.is_err());
    }

    #[test]
    fn run_on_source_with_covering_and_declining_providers_returns_diagnostics() {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::new(&ts_lang).unwrap();
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(ReportMarker)];

        let diagnostics = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[
                    &Declining as &dyn DecorationProvider,
                    &Decorating("covered"),
                ],
                &mut passes,
            )
            .expect("one covering provider should be enough");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message(), "covered");
    }

    #[test]
    fn run_on_source_with_declining_provider_returns_uncovered_file() {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::new(&ts_lang).unwrap();

        let error = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[&Declining as &dyn DecorationProvider],
                &mut Vec::new(),
            )
            .expect_err("a declining provider should not cover the file");

        let uncovered = error
            .downcast_ref::<UncoveredFile>()
            .expect("should be an UncoveredFile");
        assert_eq!(uncovered.file(), Path::new("test.rs"));
        assert_eq!(uncovered.gaps().len(), 1);
        assert_eq!(uncovered.gaps()[0].0, ProviderName("declining"));
        assert_eq!(uncovered.gaps()[0].1, CoverageGap::StaleSource);
    }

    #[test]
    fn run_on_source_with_empty_passes_produces_no_diagnostics() {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::new(&ts_lang).unwrap();

        let diagnostics = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[&Covering as &dyn DecorationProvider],
                &mut Vec::new(),
            )
            .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn run_on_source_with_lint_pass_collects_diagnostics() {
        struct AlwaysWarn;
        impl whisker_types::LintPass for AlwaysWarn {
            fn check_node(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                if node.kind() == "function_item" {
                    vec![Diagnostic::new(
                        RuleId::new("test.always"),
                        Severity::Warn,
                        "found function".into(),
                        node.span(),
                    )]
                } else {
                    Vec::new()
                }
            }
        }

        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::new(&ts_lang).unwrap();
        let mut passes: Vec<Box<dyn whisker_types::LintPass>> = vec![Box::new(AlwaysWarn)];

        let diagnostics = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[&Covering as &dyn DecorationProvider],
                &mut passes,
            )
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message(), "found function");
    }

    #[test]
    fn run_on_source_with_no_providers_returns_uncovered_file() {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::new(&ts_lang).unwrap();

        let error = pipeline
            .run_on_source("fn main() {}", Path::new("test.rs"), &[], &mut Vec::new())
            .expect_err("no providers means nothing can be analyzed");

        let uncovered = error
            .downcast_ref::<UncoveredFile>()
            .expect("should be an UncoveredFile");
        assert!(uncovered.gaps().is_empty());
        assert!(
            uncovered
                .to_string()
                .contains("no decoration providers were configured"),
            "unexpected message: {uncovered}"
        );
    }

    #[test]
    fn run_on_source_with_two_decorating_providers_keeps_first_decoration() {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::new(&ts_lang).unwrap();
        let mut passes: Vec<Box<dyn LintPass>> = vec![Box::new(ReportMarker)];

        let diagnostics = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[
                    &Decorating("first") as &dyn DecorationProvider,
                    &Decorating("second"),
                ],
                &mut passes,
            )
            .expect("both providers cover the file");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message(), "first");
    }

    #[test]
    fn run_on_source_with_uncovered_file_does_not_run_passes() {
        struct Exploding;
        impl whisker_types::LintPass for Exploding {
            fn check_node(&mut self, _node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
                panic!("lint passes must not run on an uncovered file");
            }
        }

        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::new(&ts_lang).unwrap();
        let mut passes: Vec<Box<dyn whisker_types::LintPass>> = vec![Box::new(Exploding)];

        let error = pipeline
            .run_on_source(
                "fn main() {}",
                Path::new("test.rs"),
                &[&Declining as &dyn DecorationProvider],
                &mut passes,
            )
            .expect_err("a declining provider should not cover the file");

        assert!(error.downcast_ref::<UncoveredFile>().is_some());
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn detect_language_rs_files_always_succeed(name in "[a-z]{1,10}") {
                let path = PathBuf::from(format!("{name}.rs"));
                let result = detect_language(&path);
                prop_assert!(result.is_ok());
                prop_assert_eq!(result.unwrap(), Language::Rust);
            }

            #[test]
            fn detect_language_unknown_extensions_always_fail(ext in "[a-z]{1,5}") {
                prop_assume!(ext != "rs");
                let path = PathBuf::from(format!("file.{ext}"));
                prop_assert!(detect_language(&path).is_err());
            }

            #[test]
            fn run_on_source_never_panics_on_valid_rust(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
                let mut pipeline = Pipeline::new(&ts_lang).unwrap();

                let result = pipeline.run_on_source(
                    &source,
                    Path::new("test.rs"),
                    &[&Covering as &dyn DecorationProvider],
                    &mut Vec::new(),
                );

                prop_assert!(result.is_ok());
                prop_assert!(result.unwrap().is_empty());
            }

            #[test]
            fn run_on_source_with_arbitrary_input_does_not_panic(
                source in "\\PC{0,200}",
            ) {
                let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
                let mut pipeline = Pipeline::new(&ts_lang).unwrap();

                let result = pipeline.run_on_source(
                    &source,
                    Path::new("test.rs"),
                    &[&Covering as &dyn DecorationProvider],
                    &mut Vec::new(),
                );

                prop_assert!(result.is_ok());
            }

            #[test]
            fn run_on_source_without_providers_always_reports_uncovered(
                source in "(fn [a-z]+\\(\\) \\{\\}\n){0,5}",
            ) {
                let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
                let mut pipeline = Pipeline::new(&ts_lang).unwrap();

                let result = pipeline.run_on_source(
                    &source,
                    Path::new("test.rs"),
                    &[],
                    &mut Vec::new(),
                );

                let error = result.expect_err("no providers can never cover a file");
                prop_assert!(error.downcast_ref::<UncoveredFile>().is_some());
            }
        }
    }
}
