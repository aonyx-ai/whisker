use std::path::Path;

/// Writes a lint package whose declaration is the one protocol 2 defined
///
/// The package spells the older declaration out itself, as a `#[repr(C)]`
/// struct ending after `register`, which is exactly the shape a plugin
/// built before protocol 3 exports. It cannot use whisker's own
/// [`PluginDeclaration`], because that one has the field this plugin is
/// meant to lack.
///
/// A test uses this to prove that whisker still loads such a plugin. The
/// rules of an older plugin run; they only cannot be named in `[rules]`,
/// because it declares none.
///
/// [`PluginDeclaration`]: whisker_types::plugin::PluginDeclaration
///
/// # Panics
///
/// Panics if the package cannot be written.
pub fn write_protocol_two_lint_package(directory: &Path, name: &str, rule: &str) {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let crates = std::fs::canonicalize(crates).expect("the crates directory should exist");
    let crates = crates.to_str().expect("the path should be UTF-8");

    std::fs::create_dir_all(directory.join("src")).expect("the package should be created");
    std::fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \
             \"2024\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n\
             whisker-rust = {{ path = \"{crates}/whisker-rust\", default-features = false \
             }}\nwhisker-types = {{ path = \"{crates}/whisker-types\" }}\n"
        ),
    )
    .expect("the manifest should be written");
    std::fs::write(
        directory.join("src").join("lib.rs"),
        format!(
            r#"use whisker_rust::RustLintPass;
use whisker_types::plugin::{{LintPassFactory, LintRegistrar}};
use whisker_types::{{DecoratedNode, Diagnostic, RuleId, Severity}};

pub struct Flag;

impl RustLintPass for Flag {{
    fn check_macro_invocation(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {{
        vec![Diagnostic::new(
            RuleId::new("{rule}"),
            Severity::Warn,
            "the older plugin fired".into(),
            node.span(),
        )]
    }}
}}

/// The declaration as protocol 2 defined it, ending after `register`
#[repr(C)]
pub struct DeclarationV2 {{
    pub abi_version: u32,
    pub rustc_version: *const std::ffi::c_char,
    pub types_fingerprint: u64,
    pub language_fingerprint: u64,
    pub register: fn(&mut dyn LintRegistrar),
}}

unsafe impl Sync for DeclarationV2 {{}}

fn register(registrar: &mut dyn LintRegistrar) {{
    let factory: LintPassFactory = || Box::new(whisker_rust::RustLintPassAdapter::new(Flag));
    registrar.register(factory);
}}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static whisker_plugin_declaration: DeclarationV2 = DeclarationV2 {{
    abi_version: 2,
    rustc_version: whisker_types::plugin::RUSTC_VERSION.as_ptr(),
    types_fingerprint: whisker_types::plugin::TYPES_FINGERPRINT,
    language_fingerprint: whisker_rust::plugin::LANGUAGE_FINGERPRINT,
    register,
}};
"#
        ),
    )
    .expect("the source should be written");
}
