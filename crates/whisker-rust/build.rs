use std::env;
use std::fs;
use std::path::Path;

use whisker_codegen::Fingerprint;

/// Generates the lint pass trait and fingerprints it for plugins
///
/// The fingerprint covers the generated code alone, and joins the custom
/// lint plugin handshake as half of the language fingerprint. Hashing the
/// generated trait matters: a plugin whose dispatch was generated from a
/// different grammar would not crash, its checks would just quietly never
/// fire, and the handshake turns that into a visible refusal. It stops
/// there because the rest of this crate's source does not shape the
/// boundary, and hashing it would refuse every plugin in the tree over an
/// edit that moved nothing.
fn main() {
    let node_types_json = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/node-types.json"));

    let code = whisker_codegen::generate_visitor(node_types_json, "Rust")
        .expect("failed to generate visitor from node-types.json");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("rust_lint_pass.rs");
    fs::write(&dest, &code).expect("failed to write generated visitor");

    println!("cargo::rerun-if-changed=src/node-types.json");

    let mut fingerprint = Fingerprint::new();
    fingerprint.add_bytes(code.as_bytes());
    let dest = Path::new(&out_dir).join("visitor_fingerprint.rs");
    fs::write(
        &dest,
        format!("pub const VISITOR_FINGERPRINT: u64 = 0x{fingerprint};\n"),
    )
    .expect("failed to write the visitor fingerprint");
}
