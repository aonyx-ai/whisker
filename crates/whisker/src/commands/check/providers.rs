use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use whisker_rust::RustDecorationProvider;
use whisker_types::{DecorationProvider, Language};
use whisker_typescript::TypeScriptDecorationProvider;

use super::grammar::Grammar;

/// The decoration providers a run loaded, for the languages it found
///
/// Loading a project is the expensive part of a check. rust-analyzer runs
/// build scripts, and tsrs reads every file a `tsconfig.json` names. A run
/// therefore loads a language's provider only when discovery found a file
/// in that language. A repository of one language needs no project file of
/// the other.
///
/// Rust loads one project, the workspace around the checked path. A
/// TypeScript file belongs to the project of the `tsconfig.json` nearest
/// it, as it does for `tsc` and an editor. One repository often holds
/// several, such as a documentation site beside its packages. The run loads
/// each such project once, and hands a file only the project it belongs to.
pub(super) struct Providers {
    rust: Option<RustDecorationProvider>,
    typescript: HashMap<PathBuf, TypeScriptDecorationProvider>,
}

impl Providers {
    /// Loads the providers for every language among `files`, for a check
    /// of `path`
    ///
    /// # Errors
    ///
    /// Returns an error if a needed project cannot be loaded, or if a
    /// TypeScript file has no `tsconfig.json` at or above it. The context
    /// names the language, because a mixed repository may lack only one of
    /// the project files.
    pub(super) fn load(path: &Path, files: &[PathBuf]) -> anyhow::Result<Self> {
        let mut has_rust = false;
        let mut typescript = HashMap::new();

        for file in files {
            let Some(grammar) = Grammar::of(file) else {
                continue;
            };
            match grammar.language() {
                Language::Rust => has_rust = true,
                Language::TypeScript => load_typescript_project(file, &mut typescript)
                    .context("failed to load the TypeScript project for analysis")?,
            }
        }

        let rust = match has_rust {
            true => Some(
                RustDecorationProvider::load(path)
                    .context("failed to load the target project for analysis")?,
            ),
            false => None,
        };

        Ok(Self { rust, typescript })
    }

    /// Returns the providers that may decorate `file`, a file in `language`
    ///
    /// A provider of another language, or of another TypeScript project,
    /// never sees the file. A file is therefore never reported as a gap of
    /// a project it was never part of.
    pub(super) fn for_file(&self, file: &Path, language: Language) -> Vec<&dyn DecorationProvider> {
        let provider: Option<&dyn DecorationProvider> = match language {
            Language::Rust => self
                .rust
                .as_ref()
                .map(|provider| provider as &dyn DecorationProvider),
            Language::TypeScript => TypeScriptDecorationProvider::project_root(file)
                .and_then(|root| self.typescript.get(&root))
                .map(|provider| provider as &dyn DecorationProvider),
        };
        provider.into_iter().collect()
    }
}

/// Loads the TypeScript project that holds `file`, unless it is loaded
///
/// # Errors
///
/// Returns an error if no `tsconfig.json` sits at or above `file`, or if
/// the project it names cannot be loaded.
fn load_typescript_project(
    file: &Path,
    projects: &mut HashMap<PathBuf, TypeScriptDecorationProvider>,
) -> anyhow::Result<()> {
    let root = TypeScriptDecorationProvider::project_root(file).with_context(|| {
        format!(
            "discover a TypeScript project for {}: no tsconfig.json at or above it",
            file.display()
        )
    })?;
    if projects.contains_key(&root) {
        return Ok(());
    }

    let provider = TypeScriptDecorationProvider::load(&root)
        .with_context(|| format!("load the TypeScript project at {}", root.display()))?;
    projects.insert(root, provider);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `(path, text)` files into a new temporary directory
    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        for (path, text) in files {
            let path = directory.path().join(path);
            std::fs::create_dir_all(path.parent().expect("a file has a parent"))
                .expect("directories should be created");
            std::fs::write(path, text).expect("the file should be written");
        }
        directory
    }

    #[test]
    fn for_file_routes_a_file_to_its_own_project() {
        let directory = tree(&[
            ("site/tsconfig.json", "{}"),
            ("site/page.ts", "export {};\n"),
            ("packages/a/tsconfig.json", "{}"),
            ("packages/a/index.ts", "export {};\n"),
        ]);
        let page = directory.path().join("site/page.ts");
        let index = directory.path().join("packages/a/index.ts");
        let providers = Providers::load(directory.path(), &[page.clone(), index.clone()])
            .expect("both projects should load");

        let found = providers.for_file(&page, Language::TypeScript);

        assert_eq!(providers.typescript.len(), 2);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn for_file_without_a_loaded_provider_returns_none() {
        let providers = Providers {
            rust: None,
            typescript: HashMap::new(),
        };

        let found = providers.for_file(Path::new("main.ts"), Language::TypeScript);

        assert!(found.is_empty());
    }

    #[test]
    fn load_loads_a_shared_project_once() {
        let directory = tree(&[
            ("tsconfig.json", "{}"),
            ("a.ts", "export {};\n"),
            ("b.ts", "export {};\n"),
        ]);
        let files = [directory.path().join("a.ts"), directory.path().join("b.ts")];

        let providers = Providers::load(directory.path(), &files).expect("the project should load");

        assert_eq!(providers.typescript.len(), 1);
    }

    #[test]
    fn load_with_a_typescript_file_and_no_tsconfig_names_the_file() {
        let directory = tree(&[("main.ts", "export {};\n")]);
        let file = directory.path().join("main.ts");

        let error = Providers::load(directory.path(), std::slice::from_ref(&file))
            .err()
            .expect("loading should fail");

        assert_eq!(
            error.to_string(),
            "failed to load the TypeScript project for analysis"
        );
        assert!(
            format!("{error:#}").contains(&file.display().to_string()),
            "{error:#}"
        );
    }

    #[test]
    fn load_with_no_files_loads_nothing() {
        let directory = tree(&[]);

        let providers = Providers::load(directory.path(), &[]).expect("nothing should load");

        assert!(providers.rust.is_none());
        assert!(providers.typescript.is_empty());
    }

    #[test]
    fn load_with_only_typescript_files_needs_no_cargo_project() {
        let directory = tree(&[("tsconfig.json", "{}"), ("main.ts", "export {};\n")]);
        let file = directory.path().join("main.ts");

        let providers = Providers::load(directory.path(), std::slice::from_ref(&file))
            .expect("the TypeScript project should load");

        assert!(providers.rust.is_none());
        assert_eq!(providers.for_file(&file, Language::TypeScript).len(), 1);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Providers>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Providers>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Providers>();
    }
}
