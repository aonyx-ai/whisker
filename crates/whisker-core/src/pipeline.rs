use std::path::Path;

use anyhow::Context as _;
use whisker_types::{DecoratedTree, DecorationProvider, Diagnostic, Language, LintPass};

use crate::tree_walker;

/// Orchestrates the parse-decorate-execute pipeline
///
/// The pipeline reads a source file, parses it with the appropriate
/// tree-sitter grammar, runs decoration providers, then walks the
/// decorated tree through all enabled lint passes.
// r[impl core.pipeline.parse]
// r[impl core.pipeline.decorate]
// r[impl core.pipeline.execute]
pub struct Pipeline {
    parser: tree_sitter::Parser,
}

impl Pipeline {
    /// Creates a pipeline configured for the given language
    ///
    /// # Errors
    ///
    /// Returns an error if the tree-sitter language cannot be set.
    // r[impl core.pipeline.language-detection]
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
    /// Returns an error if the file cannot be read, parsed, or decorated.
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
    /// # Errors
    ///
    /// Returns an error if parsing or decoration fails.
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

        for provider in providers {
            provider.decorate(&mut decorated)?;
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

    use whisker_types::{DecoratedNode, Diagnostic, Language, RuleId, Severity};

    use super::*;

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
    fn run_on_source_with_empty_passes_produces_no_diagnostics() {
        let ts_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut pipeline = Pipeline::new(&ts_lang).unwrap();

        let diagnostics = pipeline
            .run_on_source("fn main() {}", Path::new("test.rs"), &[], &mut Vec::new())
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
                        RuleId("test.always"),
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
            .run_on_source("fn main() {}", Path::new("test.rs"), &[], &mut passes)
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message(), "found function");
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
                    &[],
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
                    &[],
                    &mut Vec::new(),
                );

                prop_assert!(result.is_ok());
            }
        }
    }
}
