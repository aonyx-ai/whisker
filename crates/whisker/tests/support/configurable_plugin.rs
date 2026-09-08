//! A lint package whose rule reads an option before it fires
//!
//! The rule flags a macro invocation only when the project named that
//! macro in `[rules.options]`. That makes the option observable from the
//! outside: the same source and the same plugin report nothing or one
//! diagnostic, and only the configuration file differs.

use std::path::Path;

/// Writes a cdylib lint package that reads the option `macros`
///
/// The lint reports `rule` for every macro invocation whose name the
/// project listed. It declares `rule`, so a project may name it in
/// `[rules]` and in `[rules.options]`.
pub fn write_configurable_lint_package(directory: &Path, name: &str, rule: &str) {
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
    std::fs::write(directory.join("src").join("lib.rs"), source(name, rule))
        .expect("the source should be written");
}

/// Returns the source of a lint that flags the macros it was configured with
fn source(name: &str, rule: &str) -> String {
    let pass = name.to_uppercase();

    format!(
        r#"use whisker_rust::RustLintPass;
use whisker_types::{{DecoratedNode, Diagnostic, RuleId, RuleOptions, Severity}};

const RULE_ID: RuleId = RuleId::new("{rule}");

/// Flags every invocation of a macro the project named
#[derive(Default)]
pub struct {pass} {{
    macros: Vec<String>,
}}

impl RustLintPass for {pass} {{
    fn configure(&mut self, options: &RuleOptions) {{
        self.macros = options
            .names(RULE_ID, "macros")
            .unwrap_or_default()
            .to_vec();
    }}

    fn check_macro_invocation(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {{
        let Some(macro_node) = node.child_by_field_name("macro") else {{
            return Vec::new();
        }};
        if !self.macros.iter().any(|name| name == macro_node.text()) {{
            return Vec::new();
        }}

        vec![Diagnostic::new(
            RULE_ID,
            Severity::Warn,
            "this macro was named in the configuration".into(),
            node.span(),
        )]
    }}
}}

impl whisker_rust::DeclaresRules for {pass} {{
    fn rules(&self) -> Vec<RuleId> {{
        vec![RULE_ID]
    }}
}}

whisker_rust::export_lints![{pass}::default()];
"#
    )
}
