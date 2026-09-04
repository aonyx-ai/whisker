use std::collections::{BTreeSet, HashSet};
use std::ffi::{CStr, OsString, c_char};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context as _;
use libloading::Library;
use whisker_types::plugin::{LintPassFactory, LintRegistrar, PluginDeclaration};
use whisker_types::{LintPass, RuleId};

use self::handshake::AbiIdentity;
use crate::config::{GitLintSource, LintSource, WhiskerConfig};

mod abi_tag;
mod artifact;
mod cache;
mod digest;
mod git_source;
mod handshake;
mod prebuilt;

pub use abi_tag::AbiTag;

/// The lint passes loaded from the project's configured custom lint crates
///
/// Loading happens once per run, before the file walk: whisker compiles
/// each configured path with the user's cargo, opens the dynamic library
/// that build produced, and checks its declaration against this binary's
/// own ABI identity before anything in it runs. What survives is a set of
/// factories, because passes are stateful and the check command constructs
/// a fresh set for every file.
///
/// A load failure is fatal rather than recoverable: configuration that
/// silently does nothing looks exactly like configuration that works, and
/// `--keep-going` governs per-file walk errors, not a broken setup.
pub struct CustomLints {
    factories: Vec<LintPassFactory>,
    declared: Vec<RuleId>,
}

impl CustomLints {
    /// Compiles, loads, and validates every configured custom lint crate
    ///
    /// # Errors
    ///
    /// Returns an error if a configured source cannot be resolved, is
    /// configured twice, fails to build, does not export a plugin
    /// declaration, fails the ABI handshake, or registers no lints.
    pub fn load(config: &WhiskerConfig) -> anyhow::Result<Self> {
        let host = AbiIdentity::host();
        let mut factories = Vec::new();
        let mut declared = Vec::new();

        for Resolved {
            directory,
            contents,
        } in configured_sources(config, &AbiTag::host())?
        {
            let loaded = match contents {
                Contents::Sources(locking) => load_sources(&directory, locking, &host)
                    .with_context(|| {
                        format!("failed to load the custom lints at {}", directory.display())
                    }),
                Contents::Libraries => load_prebuilt(&directory, &host).with_context(|| {
                    format!(
                        "failed to load the prebuilt lints at {}",
                        directory.display()
                    )
                }),
            }?;

            factories.extend(loaded.factories);
            declared.extend(loaded.rules);
        }

        Ok(Self {
            factories,
            declared,
        })
    }

    /// Constructs one fresh pass per loaded custom lint
    ///
    /// Every pass runs. A plugin declares the rules it can report, not
    /// which pass reports which, so a rule a project turned off is
    /// dropped from the report rather than never looked for.
    pub fn instantiate(&self) -> Vec<Box<dyn LintPass>> {
        self.factories.iter().map(|factory| factory()).collect()
    }

    /// Returns every rule the loaded plugins declare
    ///
    /// This is what a configured rule name is checked against. A plugin
    /// declares its rules, so whisker knows them all before it walks a
    /// single file, and a name matching none of them is a mistake it can
    /// report rather than a filter that quietly admits everything.
    ///
    /// A plugin built against protocol 2 declares none, because its
    /// declaration ends before the field that would say. Its rules still
    /// run; they just cannot be named.
    pub fn declared(&self) -> BTreeSet<String> {
        self.declared
            .iter()
            .map(|rule| rule.as_str().to_owned())
            .collect()
    }
}

/// A resolved lint source, ready to load
///
/// Resolution loses the difference between a path and a repository. Two
/// things survive it: a directory, and whether whisker has to compile
/// what is in that directory.
#[derive(Clone, Eq, PartialEq, Debug)]
struct Resolved {
    directory: PathBuf,
    contents: Contents,
}

/// What a resolved directory holds
///
/// Both kinds end at the same place, which is a set of dynamic libraries
/// that pass the handshake. They differ in who compiled them. Whisker
/// compiles the first kind from cargo packages in the directory. A
/// publisher compiled the second kind.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Contents {
    /// Cargo packages, which whisker builds before it loads them
    Sources(Locking),

    /// Dynamic libraries, which whisker loads as they are
    Libraries,
}

/// Whether a build may change the lockfile it finds
///
/// A path source belongs to the person running whisker, and cargo updating
/// its lockfile is ordinary. A git source is pinned to a commit, and that
/// pin is only worth as much as the dependency versions behind it, so its
/// build refuses to resolve anything the committed lockfile did not
/// already settle.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Locking {
    Locked,
    Unlocked,
}

/// Resolves and validates the configured lint sources before any build runs
///
/// Everything is resolved up front, so a typo in the second entry surfaces
/// before the first entry's potentially long compilation, and a source
/// configured twice is caught however it was spelled. A git source reaches
/// the network here, which is also why it happens before the builds: a
/// network failure should not arrive after five minutes of compiling.
///
/// Whisker looks for prebuilt libraries before it takes a git source. It
/// fetches and compiles the source whenever it finds none. A project
/// that no publisher builds for therefore behaves as it did before.
///
/// # Errors
///
/// Returns an error if an entry does not exist, is not a directory, holds
/// no `Cargo.toml`, cannot be fetched, or resolves to the same directory
/// as an earlier entry.
fn configured_sources(config: &WhiskerConfig, tag: &AbiTag) -> anyhow::Result<Vec<Resolved>> {
    let mut sources = Vec::new();
    let mut seen = HashSet::new();

    for entry in config.lints() {
        let (directory, contents) = match entry {
            LintSource::Path(path) => (
                path.resolve(config.root()),
                Contents::Sources(Locking::Unlocked),
            ),
            LintSource::Git(git) => resolve_git(git, tag)?,
        };

        anyhow::ensure!(
            directory.exists(),
            "the lint source {entry} resolves to {}, which does not exist",
            directory.display()
        );
        anyhow::ensure!(
            directory.is_dir(),
            "the lint source {entry} resolves to {}, which is not a directory",
            directory.display()
        );

        let directory = std::fs::canonicalize(&directory)
            .with_context(|| format!("failed to resolve the lint source {entry}"))?;

        match contents {
            Contents::Sources(_) => anyhow::ensure!(
                directory.join("Cargo.toml").is_file(),
                "the lint source {entry} holds no Cargo.toml"
            ),
            Contents::Libraries => {}
        }

        anyhow::ensure!(
            seen.insert(directory.clone()),
            "the lint source {entry} resolves to {}, which an earlier entry already configured",
            directory.display()
        );

        sources.push(Resolved {
            directory,
            contents,
        });
    }

    Ok(sources)
}

/// Resolves a git entry, preferring whatever the machine already holds
///
/// The order is by what each answer costs. Prebuilt libraries whisker
/// already unpacked are the cheapest and win outright. A checkout that is
/// already there comes next: whisker has to compile it, but cargo has
/// compiled it before, so asking a release API first would put a network
/// request in front of a run that needs none, and every check on a train
/// would stop working. Only a machine holding neither asks anyone.
///
/// A project with a warm checkout therefore keeps compiling it, even
/// after its rules start to publish archives. A person moves the pin or
/// clears the cache to pick those up.
///
/// # Errors
///
/// Returns an error if no cache location can be determined, or if the
/// source has to be fetched and cannot be.
fn resolve_git(source: &GitLintSource, tag: &AbiTag) -> anyhow::Result<(PathBuf, Contents)> {
    if let Some(directory) = prebuilt::cached(source, tag)? {
        return Ok((directory, Contents::Libraries));
    }

    if let Some(directory) = git_source::cached(source)? {
        return Ok((directory, Contents::Sources(Locking::Locked)));
    }

    if let Some(directory) = prebuilt::fetch(source, tag)? {
        return Ok((directory, Contents::Libraries));
    }

    Ok((
        git_source::checkout(source)?,
        Contents::Sources(Locking::Locked),
    ))
}

/// Builds the packages at `directory` and loads the lints they export
///
/// A directory may hold one package or a workspace of them, so every
/// dynamic library the build produced is loaded, each with its own
/// handshake.
fn load_sources(directory: &Path, locking: Locking, host: &AbiIdentity) -> anyhow::Result<Loaded> {
    let libraries = build(directory, locking)?;

    load_libraries(&libraries, host)
}

/// Loads the lints exported by the prebuilt libraries at `directory`
///
/// Each library completes the same handshake as one whisker compiled
/// itself, and a failure fails the run. The tag whisker asked under
/// covers what the handshake compares. A library that fails therefore
/// carries a tag that misdescribes it, and a quiet compile would leave
/// that wrong for everyone who trusts the tag.
///
/// # Errors
///
/// Returns an error if the directory cannot be read, if it holds no
/// library, or if any library fails to load.
fn load_prebuilt(directory: &Path, host: &AbiIdentity) -> anyhow::Result<Loaded> {
    let libraries = prebuilt::libraries(directory)?;

    anyhow::ensure!(
        !libraries.is_empty(),
        "the directory holds no dynamic library; delete it and run whisker again to replace it"
    );

    load_libraries(&libraries, host)
}

/// Completes the handshake with each library and collects what registers
fn load_libraries(libraries: &[PathBuf], host: &AbiIdentity) -> anyhow::Result<Loaded> {
    let mut all = Loaded::default();

    for library in libraries {
        let loaded = load_library(library, host)
            .with_context(|| format!("failed to load {}", library.display()))?;
        all.factories.extend(loaded.factories);
        all.rules.extend(loaded.rules);
    }

    Ok(all)
}

/// What one library, or a directory of them, contributed
#[derive(Default)]
struct Loaded {
    factories: Vec<LintPassFactory>,
    rules: Vec<RuleId>,
}

/// Compiles the packages at `directory` and returns every library built
///
/// The build inherits stderr, so compiler errors and progress render to
/// the terminal exactly as they would for a direct `cargo build`; stdout
/// carries the JSON messages the artifact search reads. Release profile,
/// because the plugin then runs over every file of every check.
///
/// # Errors
///
/// Returns an error if cargo cannot be run, exits unsuccessfully, or
/// produces no dynamic library.
fn build(directory: &Path, locking: Locking) -> anyhow::Result<Vec<PathBuf>> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

    let mut command = Command::new(cargo);
    command.args([
        "build",
        "--release",
        "--message-format=json-render-diagnostics",
    ]);

    match locking {
        Locking::Locked => {
            command.arg("--locked");
        }
        Locking::Unlocked => {}
    }

    let output = command
        .current_dir(directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to run cargo build")?;

    anyhow::ensure!(output.status.success(), "cargo build failed");

    let stdout = String::from_utf8(output.stdout).context("cargo build wrote invalid UTF-8")?;

    artifact::cdylib_artifacts(&stdout, directory)
}

/// The first protocol version whose declaration names the plugin's rules
///
/// Whisker still loads a plugin older than this. It declares no rules, so
/// a project cannot name them in `[rules]`, and every one of them runs.
const RULES_FROM: u32 = 3;

/// Opens the built library, performs the handshake, and collects factories
///
/// The loader never forms a `&PluginDeclaration`. A plugin built against
/// an older protocol exports a shorter static, and a reference asserts
/// that the whole of the current struct is there and holds a valid value
/// of every field. Both claims are false for such a plugin, and `rules`
/// is a function pointer, so the compiler may assume it is not null. That
/// is undefined behavior whether or not the field is ever read.
///
/// So each field is read through a raw pointer at its own offset, and a
/// field is only projected once [`PluginDeclaration::abi_version`] says
/// the plugin exported it. This is what makes an appended field cheap:
/// the offsets of a `#[repr(C)]` struct are knowable per version, and no
/// step of the read depends on the plugin agreeing about the struct's
/// size.
///
/// The loader deliberately leaks the library. The registered factories and
/// the `&'static str` inside every [`RuleId`] a plugin lint mints point into
/// the library's image, so unloading it would leave dangling references
/// behind values that outlive this function. The leak is bounded by the
/// number of configured plugins in a short-lived process.
///
/// # Errors
///
/// Returns an error if the library cannot be opened, exports no plugin
/// declaration, fails the ABI handshake, or registers no lints.
///
/// [`RuleId`]: whisker_types::RuleId
fn load_library(library: &Path, host: &AbiIdentity) -> anyhow::Result<Loaded> {
    let library = unsafe { Library::new(library) }
        .with_context(|| format!("failed to open {}", library.display()))?;

    let declaration =
        unsafe { library.get::<*const PluginDeclaration>(b"whisker_plugin_declaration\0") }
            .context(
                "the library is not a whisker lint plugin; export its lints with \
             whisker_rust::export_lints!",
            )?;
    let declaration: *const PluginDeclaration = *declaration;

    let plugin_abi_version = unsafe { abi_version(declaration) };
    if !handshake::supported(plugin_abi_version) {
        return Err(handshake::HandshakeMismatch::AbiVersion {
            plugin: plugin_abi_version,
            oldest: whisker_rust::plugin::MIN_ABI_VERSION,
            newest: whisker_rust::plugin::ABI_VERSION,
        }
        .into());
    }

    let plugin = AbiIdentity {
        abi_version: plugin_abi_version,
        rustc_version: read_declaration_string(unsafe {
            (&raw const (*declaration).rustc_version).read()
        })?,
        types_fingerprint: unsafe { (&raw const (*declaration).types_fingerprint).read() },
        language_fingerprint: unsafe { (&raw const (*declaration).language_fingerprint).read() },
    };
    handshake::validate(host, &plugin)?;

    let mut registrar = Collecting {
        factories: Vec::new(),
    };
    let register = unsafe { (&raw const (*declaration).register).read() };
    register(&mut registrar);

    let rules = match plugin_abi_version >= RULES_FROM {
        true => {
            let rules = unsafe { (&raw const (*declaration).rules).read() };

            rules()
        }
        false => Vec::new(),
    };

    anyhow::ensure!(
        !registrar.factories.is_empty(),
        "the plugin registered no lints; a plugin that does nothing looks exactly like one that \
         works, so this is treated as a mistake"
    );

    std::mem::forget(library);

    Ok(Loaded {
        factories: registrar.factories,
        rules,
    })
}

/// Reads the protocol version at the head of a plugin declaration
///
/// This is the one field a plugin of any vintage can be trusted to hold,
/// because it sits at offset zero of a `#[repr(C)]` struct. Everything
/// past it, the struct's own size included, is what a matching version
/// establishes, so the read goes through a raw pointer rather than a
/// reference to the whole declaration.
///
/// # Safety
///
/// `declaration` must be the address of a loaded library's
/// `whisker_plugin_declaration` static.
unsafe fn abi_version(declaration: *const PluginDeclaration) -> u32 {
    unsafe { declaration.cast::<u32>().read_unaligned() }
}

/// Reads one C string field of a plugin declaration
///
/// # Errors
///
/// Returns an error if the pointer is null or the string is not UTF-8,
/// both of which mean the declaration was not written by `export_lints!`.
fn read_declaration_string(field: *const c_char) -> anyhow::Result<String> {
    anyhow::ensure!(
        !field.is_null(),
        "the plugin declaration is malformed; export lints with whisker_rust::export_lints!"
    );

    let text = unsafe { CStr::from_ptr(field) };
    let text = text
        .to_str()
        .context("the plugin declaration is malformed; export lints with export_lints!")?;

    Ok(text.to_owned())
}

/// Gathers the factories a plugin registers
struct Collecting {
    factories: Vec<LintPassFactory>,
}

impl LintRegistrar for Collecting {
    fn register(&mut self, factory: LintPassFactory) {
        self.factories.push(factory);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use kawauso_project::project::ProjectRoot;

    use super::*;
    use crate::config::LintPath;

    fn config_with(root: &Path, lints: Vec<LintPath>) -> WhiskerConfig {
        let lints = lints.into_iter().map(LintSource::Path).collect();

        WhiskerConfig::new(ProjectRoot::new(root.to_path_buf()), Vec::new(), lints)
    }

    fn directories(config: &WhiskerConfig) -> anyhow::Result<Vec<PathBuf>> {
        let sources = configured_sources(config, &AbiTag::host())?;

        Ok(sources
            .into_iter()
            .map(|Resolved { directory, .. }| directory)
            .collect())
    }

    fn package(root: &Path, name: &str) -> PathBuf {
        let directory = root.join(name);
        std::fs::create_dir_all(&directory).expect("package directory should be created");
        std::fs::write(directory.join("Cargo.toml"), "[package]\n").expect("manifest");
        directory
    }

    #[test]
    fn abi_version_reads_a_declaration_that_ends_after_the_version() {
        #[repr(C)]
        struct Truncated {
            abi_version: u32,
        }
        let truncated = Truncated { abi_version: 7 };

        let version = unsafe { abi_version((&raw const truncated).cast::<PluginDeclaration>()) };

        assert_eq!(version, 7);
    }

    #[test]
    fn configured_sources_rejects_a_duplicate_entry() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        package(root.path(), "no_todo");
        let config = config_with(
            root.path(),
            vec![LintPath::new("no_todo"), LintPath::new("./no_todo")],
        );

        let error = directories(&config).expect_err("should fail");

        assert!(
            error.to_string().contains("already configured"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn configured_sources_rejects_a_file_entry() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::write(root.path().join("lint.rs"), "").expect("file should be written");
        let config = config_with(root.path(), vec![LintPath::new("lint.rs")]);

        let error = directories(&config).expect_err("should fail");

        assert!(
            error.to_string().contains("not a directory"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn configured_sources_rejects_a_missing_entry() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        let config = config_with(root.path(), vec![LintPath::new("absent")]);

        let error = directories(&config).expect_err("should fail");

        assert!(
            error.to_string().contains("does not exist"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn configured_sources_rejects_an_entry_without_manifest() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::create_dir(root.path().join("empty")).expect("directory should be created");
        let config = config_with(root.path(), vec![LintPath::new("empty")]);

        let error = directories(&config).expect_err("should fail");

        assert!(
            error.to_string().contains("no Cargo.toml"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn configured_sources_resolves_entries_in_order() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        let first = package(root.path(), "first");
        let second = package(root.path(), "second");
        let config = config_with(
            root.path(),
            vec![LintPath::new("first"), LintPath::new("second")],
        );

        let resolved = directories(&config).expect("should resolve");

        assert_eq!(
            resolved,
            vec![
                std::fs::canonicalize(first).expect("should resolve"),
                std::fs::canonicalize(second).expect("should resolve"),
            ]
        );
    }

    #[test]
    fn configured_sources_leaves_a_path_source_unlocked() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        package(root.path(), "no_todo");
        let config = config_with(root.path(), vec![LintPath::new("no_todo")]);

        let sources = configured_sources(&config, &AbiTag::host()).expect("should resolve");

        assert_eq!(
            sources.first().map(|source| source.contents),
            Some(Contents::Sources(Locking::Unlocked)),
            "a lockfile on disk belongs to the person running whisker"
        );
    }

    #[test]
    fn load_with_no_configured_lints_yields_no_passes() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        let config = config_with(root.path(), Vec::new());

        let lints = CustomLints::load(&config).expect("should load");

        assert!(lints.instantiate().is_empty());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CustomLints>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CustomLints>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CustomLints>();
    }
}
