use std::path::Path;

use whisker_core::Pipeline;
use whisker_types::Language;

/// The grammar a file parses with, which its extension decides
///
/// A language can need more than one grammar. TypeScript does: `.tsx`
/// files parse with a grammar that knows JSX, and that grammar cannot read
/// the angle-bracket type assertion a `.ts` file may hold. Each grammar
/// therefore gets its own pipeline, while the trees of both carry
/// [`Language::TypeScript`], so the same lint passes and the same
/// provider see them.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(super) enum Grammar {
    /// Rust, for `.rs` files
    Rust,
    /// TypeScript without JSX, for `.ts`, `.mts`, and `.cts` files
    TypeScript,
    /// TypeScript with JSX, for `.tsx` files
    Tsx,
}

impl Grammar {
    /// Returns the grammar for the file at `path`, or [`None`] when whisker
    /// has none for its extension
    ///
    /// Discovery admits only the files [`Language::from_extension`]
    /// recognizes, and this agrees with it extension for extension.
    pub(super) fn of(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?;
        match extension {
            "rs" => Some(Self::Rust),
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            _ => None,
        }
    }

    /// Returns the language every tree of this grammar carries
    pub(super) fn language(self) -> Language {
        match self {
            Self::Rust => Language::Rust,
            Self::TypeScript => Language::TypeScript,
            Self::Tsx => Language::TypeScript,
        }
    }

    /// Builds a pipeline that parses with this grammar
    ///
    /// # Errors
    ///
    /// Returns an error if tree-sitter refuses the grammar.
    pub(super) fn pipeline(self) -> anyhow::Result<Pipeline> {
        let grammar = match self {
            Self::Rust => whisker_rust::language(),
            Self::TypeScript => whisker_typescript::language(),
            Self::Tsx => whisker_typescript::tsx_language(),
        };
        Pipeline::new(self.language(), &grammar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_of_tsx_is_typescript() {
        let language = Grammar::Tsx.language();

        assert_eq!(language, Language::TypeScript);
    }

    #[test]
    fn of_agrees_with_language_from_extension() {
        for extension in ["rs", "ts", "mts", "cts", "tsx", "js", "py", "d", ""] {
            let path = format!("file.{extension}");

            let grammar = Grammar::of(Path::new(&path)).map(Grammar::language);

            assert_eq!(
                grammar,
                Language::from_extension(extension),
                "`.{extension}` should pick a grammar of the language discovery admits"
            );
        }
    }

    #[test]
    fn of_without_an_extension_returns_none() {
        let grammar = Grammar::of(Path::new("Makefile"));

        assert_eq!(grammar, None);
    }

    #[test]
    fn of_with_tsx_returns_the_tsx_grammar() {
        let grammar = Grammar::of(Path::new("src/view.tsx"));

        assert_eq!(grammar, Some(Grammar::Tsx));
    }

    #[test]
    fn pipeline_builds_for_every_grammar() {
        for grammar in [Grammar::Rust, Grammar::TypeScript, Grammar::Tsx] {
            let pipeline = grammar.pipeline();

            assert!(pipeline.is_ok(), "{grammar:?} should build a pipeline");
        }
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Grammar>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Grammar>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Grammar>();
    }
}
