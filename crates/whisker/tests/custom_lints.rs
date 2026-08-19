use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;
use whisker_rust::plugin::LANGUAGE_FINGERPRINT;
use whisker_types::plugin::{ABI_VERSION, RUSTC_VERSION, TYPES_FINGERPRINT};

/// Source that trips the example's syntactic lint and none of the built-ins
const TODO_SOURCE: &str = "pub fn later() {\n    todo!()\n}\n";

/// Source whose fallibility only a resolved signature can see
///
/// The alias hides the `Result` from anything reading the syntax, so the
/// example's decoration-reading lint fires here and a syntactic one could
/// not.
const ALIASED_RESULT_SOURCE: &str = "pub type Fallible = std::result::Result<(), std::io::Error>;\n\npub fn save() -> Fallible {\n    Ok(())\n}\n";

/// A manifest for a standalone package with no dependencies
const MANIFEST: &str = "[package]\nname = \"target\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// Creates a minimal Cargo package whose `src/lib.rs` holds `source`
///
/// A real package is the only target rust-analyzer can load, exactly as in
/// the check tests.
fn package(source: &str) -> TempDir {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    std::fs::write(directory.path().join("Cargo.toml"), MANIFEST)
        .expect("manifest should be written");
    std::fs::create_dir(directory.path().join("src")).expect("src should be created");
    std::fs::write(directory.path().join("src").join("lib.rs"), source)
        .expect("source should be written");

    directory
}

/// Points the package's whisker configuration at one custom lint path
///
/// The path goes through a TOML value rather than into quotes of our own,
/// so a checkout under a directory holding a quote still writes a
/// configuration whisker can read.
fn configure_lint(package: &Path, lint_path: &Path) {
    let lint_path = lint_path.to_str().expect("the lint path should be UTF-8");
    let lint_path = toml::Value::from(lint_path);

    std::fs::write(
        package.join(".whisker.toml"),
        format!("[[lints]]\npath = {lint_path}\n"),
    )
    .expect("configuration should be written");
}

/// Returns the example plugin this repository ships
fn example_lint() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/custom_lint");
    std::fs::canonicalize(path).expect("the example lint should exist")
}

/// Returns the whisker-types this test binary was compiled against
fn whisker_types() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../whisker-types");
    std::fs::canonicalize(path).expect("whisker-types should exist")
}

/// Returns a whisker command that builds its plugins in the usual place
///
/// The plugin builds here deliberately inherit the environment rather than
/// sharing one target directory: whisker's cargo is not the only one that
/// reads `CARGO_TARGET_DIR`, and rust-analyzer would load every checked
/// package into that directory too. Two temporary packages of one name
/// would then write over each other's artifacts.
fn whisker() -> Command {
    Command::cargo_bin("whisker").expect("whisker binary should exist")
}

/// Renders the expression a fixture uses for one C string field
fn c_string(text: &CStr) -> String {
    let text = text.to_str().expect("the host's constants should be UTF-8");
    let text = text.replace('\\', "\\\\").replace('"', "\\\"");

    format!("c\"{text}\".as_ptr()")
}

/// How much of the declaration a fixture exports
enum Shape {
    /// Every field the protocol declares today
    Full,
    /// The protocol version, and nothing after it
    VersionOnly,
}

/// A plugin declaration written field by field
///
/// `export_lints!` cannot produce a declaration that fails the handshake,
/// which leaves every refusal in the loader untested. A fixture writes the
/// declaration itself instead: it declares its own `#[repr(C)]` struct,
/// depends on nothing, and so can state what no real plugin would.
///
/// The defaults are the host's own constants, so a fixture loads unless a
/// test spoils exactly one of them, and the refusal a test observes is the
/// one that field is responsible for.
struct RawPlugin {
    name: &'static str,
    preamble: String,
    abi_version: String,
    rustc_version: String,
    types_fingerprint: String,
    language_fingerprint: String,
    shape: Shape,
}

impl RawPlugin {
    /// Creates a fixture whose declaration matches this whisker exactly
    ///
    /// The name reaches the fixture's package name, so a cargo message
    /// about the build names the fixture that produced it.
    fn new(name: &'static str) -> Self {
        Self {
            name,
            preamble: String::new(),
            abi_version: ABI_VERSION.to_string(),
            rustc_version: c_string(RUSTC_VERSION),
            types_fingerprint: c_string(TYPES_FINGERPRINT),
            language_fingerprint: c_string(LANGUAGE_FINGERPRINT),
            shape: Shape::Full,
        }
    }

    /// Renders the crate root that exports the declaration
    fn source(&self) -> String {
        let Self {
            preamble,
            abi_version,
            rustc_version,
            types_fingerprint,
            language_fingerprint,
            ..
        } = self;

        match self.shape {
            Shape::Full => format!(
                r#"use std::ffi::c_char;

{preamble}#[repr(C)]
pub struct PluginDeclaration {{
    pub abi_version: u32,
    pub rustc_version: *const c_char,
    pub types_fingerprint: *const c_char,
    pub language_fingerprint: *const c_char,
    pub register: fn(&mut ()),
}}

unsafe impl Sync for PluginDeclaration {{}}

fn register(_registrar: &mut ()) {{}}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static whisker_plugin_declaration: PluginDeclaration = PluginDeclaration {{
    abi_version: {abi_version},
    rustc_version: {rustc_version},
    types_fingerprint: {types_fingerprint},
    language_fingerprint: {language_fingerprint},
    register,
}};
"#
            ),
            Shape::VersionOnly => format!(
                r#"#[repr(C)]
pub struct PluginDeclaration {{
    pub abi_version: u32,
}}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static whisker_plugin_declaration: PluginDeclaration =
    PluginDeclaration {{ abi_version: {abi_version} }};
"#
            ),
        }
    }

    /// Writes the fixture as a cdylib package and returns its directory
    fn write(&self) -> TempDir {
        write_plugin(self.name, "", &self.source())
    }
}

/// Writes a cdylib package holding `source` and returns its directory
fn write_plugin(name: &str, dependencies: &str, source: &str) -> TempDir {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    std::fs::write(
        directory.path().join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n{dependencies}"
        ),
    )
    .expect("manifest should be written");
    std::fs::create_dir(directory.path().join("src")).expect("src should be created");
    std::fs::write(directory.path().join("src").join("lib.rs"), source)
        .expect("source should be written");

    directory
}

/// Writes a plugin that passes the handshake and then registers no lints
///
/// This one cannot be a [`RawPlugin`]: the registrar it has to call is a
/// whisker type, so the fixture depends on whisker-types and builds its
/// declaration from the same constants a real plugin would. Only the
/// language fingerprint is written in, because whisker-rust owns that one
/// and a lint that registers nothing needs nothing else of it.
fn plugin_registering_nothing() -> TempDir {
    let types = whisker_types();
    let types = toml::Value::from(types.to_str().expect("the path should be UTF-8"));
    let language = LANGUAGE_FINGERPRINT
        .to_str()
        .expect("the host's constants should be UTF-8");

    write_plugin(
        "registers_nothing",
        &format!("\n[dependencies]\nwhisker-types = {{ path = {types} }}\n"),
        &format!(
            r#"use std::ffi::CStr;

use whisker_types::plugin::{{LintRegistrar, PluginDeclaration, c_str}};

const LANGUAGE_FINGERPRINT: &CStr = c_str("{language}\0");

fn register(_registrar: &mut dyn LintRegistrar) {{}}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static whisker_plugin_declaration: PluginDeclaration = PluginDeclaration {{
    abi_version: whisker_types::plugin::ABI_VERSION,
    rustc_version: whisker_types::plugin::RUSTC_VERSION.as_ptr(),
    types_fingerprint: whisker_types::plugin::TYPES_FINGERPRINT.as_ptr(),
    language_fingerprint: LANGUAGE_FINGERPRINT.as_ptr(),
    register,
}};
"#
        ),
    )
}

/// Pins that a plugin reads the decorations the host attached
///
/// The lint fires on a fact the syntax does not carry: the function's
/// return type is an alias, so only the signature the provider resolved
/// names a `Result` at all. A plugin that could not read the host's
/// decorations would report nothing here and pass silently, which is the
/// failure this test exists to catch.
#[test]
fn check_with_custom_lint_reading_a_decoration_reports_its_diagnostic() {
    let target = package(ALIASED_RESULT_SOURCE);
    configure_lint(target.path(), &example_lint());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[custom.anyhow-error]"))
        .stderr(predicate::str::contains("return anyhow::Error"));
}

/// Pins the whole path: configure, compile, handshake, lint, report
///
/// One toolchain builds both sides here, as the setup documentation asks
/// of a real user, so the handshake passes for the same reason theirs
/// does.
#[test]
fn check_with_custom_lint_reports_its_diagnostic() {
    let target = package(TODO_SOURCE);
    configure_lint(target.path(), &example_lint());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[custom.no-todo]"))
        .stderr(predicate::str::contains("finish this before it ships"));
}

/// Pins the order the loader reads a declaration in
///
/// This fixture's declaration holds its protocol version and stops. Every
/// other field the current protocol declares is absent, so a loader that
/// took a reference to the whole struct before comparing versions would be
/// reading memory the plugin never wrote. The refusal below is what says
/// it read the version first.
#[test]
fn check_with_declaration_that_ends_after_its_version_fails() {
    let mut plugin = RawPlugin::new("ends_after_version");
    plugin.abi_version = "99".to_string();
    plugin.shape = Shape::VersionOnly;
    let plugin = plugin.write();

    let target = package(TODO_SOURCE);
    configure_lint(target.path(), plugin.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the plugin speaks declaration protocol version 99",
        ));
}

/// Pins the refusal for a declaration whose strings are not readable
#[test]
fn check_with_declaration_whose_string_is_not_utf8_fails() {
    let mut plugin = RawPlugin::new("string_is_not_utf8");
    plugin.preamble = "static INVALID_UTF8: [u8; 3] = [0xff, 0xfe, 0];\n\n".to_string();
    plugin.rustc_version = "(&raw const INVALID_UTF8).cast::<c_char>()".to_string();
    let plugin = plugin.write();

    let target = package(TODO_SOURCE);
    configure_lint(target.path(), plugin.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the plugin declaration is malformed",
        ));
}

/// Pins the refusal for a declaration field that points nowhere
#[test]
fn check_with_declaration_whose_string_is_null_fails() {
    let mut plugin = RawPlugin::new("string_is_null");
    plugin.rustc_version = "std::ptr::null()".to_string();
    let plugin = plugin.write();

    let target = package(TODO_SOURCE);
    configure_lint(target.path(), plugin.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the plugin declaration is malformed",
        ));
}

#[test]
fn check_with_lint_path_that_does_not_exist_fails() {
    let target = package(TODO_SOURCE);
    configure_lint(target.path(), Path::new("does/not/exist"));

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "failed to load the project's custom lints",
        ))
        .stderr(predicate::str::contains("does not exist"));
}

/// Pins the remedy for the most likely authoring mistake
///
/// The decoy lint package has no dependencies, so the test builds it in
/// about a second and never gets as far as loading it.
#[test]
fn check_with_lint_that_builds_no_cdylib_fails() {
    let target = package(TODO_SOURCE);
    let lint = package("pub fn not_a_lint() {}");
    configure_lint(target.path(), lint.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("crate-type = [\"cdylib\"]"));
}

/// Pins the error for a dynamic library that is not a whisker plugin
#[test]
fn check_with_lint_that_exports_no_declaration_fails() {
    let target = package(TODO_SOURCE);
    let lint = package("pub fn not_a_lint() {}");
    std::fs::write(
        lint.path().join("Cargo.toml"),
        format!("{MANIFEST}\n[lib]\ncrate-type = [\"cdylib\"]\n"),
    )
    .expect("manifest should be written");
    configure_lint(target.path(), lint.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a whisker lint plugin"))
        .stderr(predicate::str::contains("export_lints!"));
}

/// Pins that a plugin doing nothing is a mistake rather than a no-op
///
/// `export_lints!` takes at least one lint, so only a hand-written
/// declaration can reach this refusal.
#[test]
fn check_with_lint_that_registers_nothing_fails() {
    let plugin = plugin_registering_nothing();
    let target = package(TODO_SOURCE);
    configure_lint(target.path(), plugin.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("registered no lints"));
}

/// Pins the refusal for a plugin built against another whisker-rust
#[test]
fn check_with_plugin_built_against_another_language_crate_fails() {
    let mut plugin = RawPlugin::new("other_language_crate");
    plugin.language_fingerprint = "c\"0000000000000000\".as_ptr()".to_string();
    let plugin = plugin.write();

    let target = package(TODO_SOURCE);
    configure_lint(target.path(), plugin.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "built against a different revision of whisker-rust",
        ));
}

/// Pins the refusal for a plugin built against another whisker-types
#[test]
fn check_with_plugin_built_against_another_types_crate_fails() {
    let mut plugin = RawPlugin::new("other_types_crate");
    plugin.types_fingerprint = "c\"0000000000000000\".as_ptr()".to_string();
    let plugin = plugin.write();

    let target = package(TODO_SOURCE);
    configure_lint(target.path(), plugin.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "built against a different revision of whisker-types",
        ));
}

/// Pins the refusal for a plugin another compiler built
#[test]
fn check_with_plugin_built_by_another_compiler_fails() {
    let mut plugin = RawPlugin::new("other_compiler");
    plugin.rustc_version = "c\"rustc 0.0.0-nightly (0000000 2000-01-01)\".as_ptr()".to_string();
    let plugin = plugin.write();

    let target = package(TODO_SOURCE);
    configure_lint(target.path(), plugin.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("rustc 0.0.0-nightly"))
        .stderr(predicate::str::contains("same toolchain"));
}

/// Pins the refusal for a plugin speaking another declaration protocol
#[test]
fn check_with_plugin_of_another_protocol_version_fails() {
    let mut plugin = RawPlugin::new("other_protocol");
    plugin.abi_version = "99".to_string();
    let plugin = plugin.write();

    let target = package(TODO_SOURCE);
    configure_lint(target.path(), plugin.path());

    whisker()
        .arg("check")
        .arg(target.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the plugin speaks declaration protocol version 99",
        ))
        .stderr(predicate::str::contains("rebuild the plugin"));
}
