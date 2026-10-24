use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use tsrs_compiler::{Program, ProgramOptions, new_cached_fs_compiler_host, new_program};
use tsrs_core::P;
use tsrs_core::context::Context;
use tsrs_tsoptions::{ParseConfigHost, ParsedCommandLine};
use tsrs_vfs::{FS, bundled, osvfs};
use whisker_types::{
    Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationMap, DecorationProvider,
    ProviderName,
};

use crate::decorations::TypeText;

/// The file that marks the root of a TypeScript project
const CONFIG_FILE: &str = "tsconfig.json";

/// The node kinds that name a value the checker can type
const TYPED_KINDS: [&str; 3] = [
    "identifier",
    "property_identifier",
    "shorthand_property_identifier",
];

/// Decorates TypeScript trees with what the TypeScript checker knows
///
/// The provider loads one TypeScript project through tsrs, a Rust port of
/// the TypeScript 7 checker, and answers for the files that project's
/// program holds. It attaches a [`TypeText`] to each identifier the
/// checker can type.
///
/// The provider declines a file the program does not hold, with the gap
/// that leads to its remedy:
///
/// - [`CoverageGap::OutsideRoot`] for a file outside the project directory.
/// - [`CoverageGap::ExcludedByToolchain`] for a file that an `include`
///   pattern matches and `exclude` removed.
/// - [`CoverageGap::Unreachable`] for any other file.
///
/// A file whose text changed after the load is [`CoverageGap::StaleSource`],
/// because the checker's offsets would address the old text.
///
/// The program is built once and lives for the rest of the process, as it
/// does in tsrs's own command-line compiler. Whisker loads one project per
/// run, so the memory is the run's.
///
/// # Examples
///
/// ```ignore
/// let provider = TypeScriptDecorationProvider::load(Path::new("."))?;
/// let coverage = provider.decorate(&tree)?;
/// ```
pub struct TypeScriptDecorationProvider {
    program: &'static Program,
    config: P<ParsedCommandLine>,
    root: Arc<Path>,
}

impl TypeScriptDecorationProvider {
    /// Identifies this provider in diagnostics
    const NAME: ProviderName = ProviderName("typescript");

    /// Loads the TypeScript project that holds `path`
    ///
    /// The project is the one whose `tsconfig.json` is nearest at or above
    /// `path`, as `tsc` finds it. Its directory becomes the provider's
    /// root, which a gap names.
    ///
    /// # Errors
    ///
    /// Returns an error if the current directory cannot be read, or if no
    /// `tsconfig.json` sits at or above `path`. It also returns one if a path
    /// is not valid UTF-8, or if the configuration cannot be parsed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// use whisker_typescript::TypeScriptDecorationProvider;
    ///
    /// let provider = TypeScriptDecorationProvider::load(Path::new("."))?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let path = std::path::absolute(path)
            .with_context(|| format!("resolve {} to an absolute path", path.display()))?;
        let config_file = discover_config(&path).with_context(|| {
            format!(
                "discover a TypeScript project at {}: no {CONFIG_FILE} at or above it",
                path.display()
            )
        })?;
        let root: Arc<Path> = Arc::from(
            config_file
                .parent()
                .context("a configuration file always sits in a directory")?,
        );
        let root_text = root.to_str().with_context(|| not_utf8(&root))?;
        let config_text = config_file
            .to_str()
            .with_context(|| not_utf8(&config_file))?;

        let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(osvfs::fs()));
        let host = ConfigHost {
            fs: Arc::clone(&fs),
            current_directory: root_text.to_owned(),
        };
        let (config, errors) = tsrs_tsoptions::get_parsed_command_line_of_config_file(
            config_text,
            None,
            None,
            &host,
            None,
        );
        anyhow::ensure!(
            errors.is_empty(),
            "parse {}: {}",
            config_file.display(),
            errors
                .iter()
                .map(|error| error.localize())
                .collect::<Vec<_>>()
                .join("; ")
        );
        let config = P::new(config.with_context(|| format!("parse {}", config_file.display()))?);

        let compiler_host = new_cached_fs_compiler_host(
            root_text,
            fs,
            &bundled::default_lib_path(),
            None,
            None,
            None,
        );
        let program = new_program(ProgramOptions::new(config, compiler_host));

        Ok(Self {
            program,
            config,
            root,
        })
    }

    /// Returns the gap for a file the program does not hold
    fn gap_for_unknown_file(&self, file: &Path) -> CoverageGap {
        let root = Arc::clone(&self.root);

        if !file.starts_with(&self.root) {
            return CoverageGap::OutsideRoot { root };
        }

        let Some(file) = file.to_str() else {
            return CoverageGap::Unreachable { root };
        };
        let (include, _) = self.config.get_matched_include_spec(file);
        match include.is_empty() {
            true => CoverageGap::Unreachable { root },
            false => CoverageGap::ExcludedByToolchain { root },
        }
    }
}

impl DecorationProvider for TypeScriptDecorationProvider {
    fn name(&self) -> ProviderName {
        Self::NAME
    }

    fn decorate(&self, tree: &DecoratedTree) -> anyhow::Result<Coverage> {
        let file = tree.file();
        let file = std::path::absolute(file)
            .with_context(|| format!("resolve {} to an absolute path", file.display()))?;

        let Some(source_file) = file
            .to_str()
            .and_then(|name| self.program.get_source_file(name))
        else {
            return Ok(Coverage::NotCovered(self.gap_for_unknown_file(&file)));
        };

        if source_file.text() != tree.source() {
            return Ok(Coverage::NotCovered(CoverageGap::StaleSource));
        }

        let mut targets = Vec::new();
        collect_targets(&tree.root_node(), &mut targets);

        let context = Context::background();
        let mut checker = self
            .program
            .get_type_checker_for_file(&context, source_file);
        let mut decorations = DecorationMap::new();

        for Target {
            node_id,
            start,
            end,
        } in targets
        {
            let token = tsrs_astnav::get_touching_property_name(source_file, start);
            if token.end().source_offset() != Some(end) {
                continue;
            }

            let ty = checker.get_type_at_location(token);
            let text = checker.type_to_string_exported(ty);
            decorations.insert(node_id, TypeText::new(&text));
        }

        Ok(Coverage::Covered(decorations))
    }
}

/// An identifier the checker is asked to type, by its byte range
struct Target {
    node_id: usize,
    start: u32,
    end: u32,
}

/// Collects every node of a [`TYPED_KINDS`] kind under `node`
fn collect_targets(node: &DecoratedNode<'_>, targets: &mut Vec<Target>) {
    if TYPED_KINDS.contains(&node.kind())
        && let Ok(start) = u32::try_from(node.raw().start_byte())
        && let Ok(end) = u32::try_from(node.raw().end_byte())
    {
        targets.push(Target {
            node_id: node.id(),
            start,
            end,
        });
    }

    for child in node.named_children() {
        collect_targets(&child, targets);
    }
}

/// Returns the `tsconfig.json` nearest at or above `path`
fn discover_config(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .map(|directory| directory.join(CONFIG_FILE))
        .find(|candidate| candidate.is_file())
}

/// Returns the error for a path tsrs cannot address, because tsrs
/// addresses every file by UTF-8 text
fn not_utf8(path: &Path) -> String {
    format!(
        "{} is not valid UTF-8, and tsrs only addresses UTF-8 paths",
        path.display()
    )
}

/// The file system and directory tsrs reads a configuration through
struct ConfigHost {
    fs: Arc<dyn FS>,
    current_directory: String,
}

impl ParseConfigHost for ConfigHost {
    fn fs(&self) -> &dyn FS {
        &*self.fs
    }

    fn get_current_directory(&self) -> &str {
        &self.current_directory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_utf8_names_the_path() {
        let message = not_utf8(Path::new("/projects/app"));

        assert!(message.starts_with("/projects/app is not valid UTF-8"));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TypeScriptDecorationProvider>();
        assert_send::<Target>();
        assert_send::<ConfigHost>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TypeScriptDecorationProvider>();
        assert_sync::<Target>();
        assert_sync::<ConfigHost>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TypeScriptDecorationProvider>();
        assert_unpin::<Target>();
        assert_unpin::<ConfigHost>();
    }
}
