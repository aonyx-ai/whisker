use std::env;
use std::fs;
use std::path::Path;

use whisker_codegen::Fingerprint;

/// Generates the lint pass trait and fingerprints the crate for plugins
///
/// The fingerprint covers the sources and the generated code, and joins
/// the custom lint plugin handshake as the language fingerprint. Hashing
/// the generated trait matters: a plugin whose dispatch was generated from
/// a different grammar would not crash, its checks would just quietly
/// never fire, and the handshake turns that into a visible refusal.
fn main() {
    let node_types_json = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/node-types.json"));

    let code = whisker_codegen::generate_visitor(node_types_json, "Rust")
        .expect("failed to generate visitor from node-types.json");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("rust_lint_pass.rs");
    fs::write(&dest, &code).expect("failed to write generated visitor");

    println!("cargo::rerun-if-changed=src");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("cargo should set CARGO_MANIFEST_DIR");
    let mut fingerprint = Fingerprint::new();
    fingerprint
        .add_directory(&Path::new(&manifest_dir).join("src"))
        .expect("failed to read the crate sources");
    fingerprint.add_bytes(code.as_bytes());
    println!("cargo::rustc-env=WHISKER_RUST_FINGERPRINT={fingerprint}");
}
