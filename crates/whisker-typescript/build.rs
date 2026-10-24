use std::env;
use std::fs;
use std::path::Path;

use whisker_codegen::Fingerprint;

/// Generates the lint pass trait and fingerprints it for plugins
///
/// TypeScript parses with two grammars: `.tsx` files need one that knows
/// JSX, which in turn drops the angle-bracket type assertion. The trait is
/// generated from the union of both grammars' node kinds, so one rule runs
/// on either kind of file.
///
/// The fingerprint covers the generated code alone, and joins the custom
/// lint plugin handshake through whisker-plugin. A plugin whose dispatch
/// came from a different grammar would not crash. Its checks would just
/// never fire, and the handshake turns that into a visible refusal.
fn main() {
    let typescript = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/typescript-node-types.json"
    ));
    let tsx = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tsx-node-types.json"
    ));

    let node_types = whisker_codegen::merge_node_types(&[typescript, tsx])
        .expect("failed to merge the TypeScript and TSX node types");
    let code = whisker_codegen::generate_visitor(&node_types, "TypeScript")
        .expect("failed to generate visitor from node-types.json");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("typescript_lint_pass.rs");
    fs::write(&dest, &code).expect("failed to write generated visitor");

    println!("cargo::rerun-if-changed=src/typescript-node-types.json");
    println!("cargo::rerun-if-changed=src/tsx-node-types.json");

    let mut fingerprint = Fingerprint::new();
    fingerprint.add_bytes(code.as_bytes());
    let dest = Path::new(&out_dir).join("visitor_fingerprint.rs");
    fs::write(
        &dest,
        format!("pub const VISITOR_FINGERPRINT: u64 = 0x{fingerprint};\n"),
    )
    .expect("failed to write the visitor fingerprint");
}
