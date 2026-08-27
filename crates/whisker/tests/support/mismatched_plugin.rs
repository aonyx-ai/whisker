use std::path::Path;

use crate::fixture_repository::whisker_crates;

/// Writes a package whose declaration claims a protocol whisker refuses
///
/// The library it builds is a real one: whisker can open it and find the
/// declaration it looks for. Only the protocol version is wrong, which is
/// the first thing the handshake compares, so the refusal comes from the
/// handshake rather than from the loader failing earlier.
///
/// A test uses this to prove that a library whisker did not compile
/// itself is held to the same standard as one it did.
///
/// # Panics
///
/// Panics if the package cannot be written.
pub fn write_mismatched_lint_package(directory: &Path, name: &str) {
    let crates = whisker_crates();
    let crates = crates.to_str().expect("the path should be UTF-8");

    std::fs::create_dir_all(directory.join("src")).expect("the package should be created");
    std::fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \
             \"2024\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n\
             whisker-types = {{ path = \"{crates}/whisker-types\" }}\n"
        ),
    )
    .expect("the manifest should be written");
    std::fs::write(
        directory.join("src").join("lib.rs"),
        r#"use whisker_types::plugin::{LintRegistrar, PluginDeclaration};

/// Registers no lint, because the handshake refuses this library first
fn register(_registrar: &mut dyn LintRegistrar) {}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static whisker_plugin_declaration: PluginDeclaration = PluginDeclaration {
    abi_version: whisker_types::plugin::ABI_VERSION + 1,
    rustc_version: whisker_types::plugin::RUSTC_VERSION.as_ptr(),
    types_fingerprint: whisker_types::plugin::TYPES_FINGERPRINT,
    language_fingerprint: 0,
    register,
};
"#,
    )
    .expect("the source should be written");
}
