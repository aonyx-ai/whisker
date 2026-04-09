use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let node_types_json = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/node-types.json"));

    let code = whisker_macros::generate_visitor(node_types_json, "Rust")
        .expect("failed to generate visitor from node-types.json");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("rust_lint_pass.rs");
    fs::write(&dest, code).expect("failed to write generated visitor");

    println!("cargo::rerun-if-changed=src/node-types.json");
}
