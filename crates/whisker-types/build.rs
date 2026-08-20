use std::path::Path;

use whisker_codegen::Fingerprint;

/// Bakes the compiler's identity and a source fingerprint into the crate
///
/// Both values feed the custom lint plugin handshake. Rust has no stable
/// ABI, so whisker only loads a plugin whose whisker-types was compiled by
/// the same rustc from the same source as its own. The crate version cannot
/// stand in for the source: every whisker crate is unpublished and stays at
/// one version between releases, so two builds of "0.1.0" can differ. The
/// fingerprint detects drift; it is not a security measure.
fn main() {
    println!("cargo::rerun-if-changed=src");

    let version = rustc_version::version_meta().expect("failed to read the rustc version");
    println!(
        "cargo::rustc-env=WHISKER_RUSTC_VERSION={}",
        version.short_version_string
    );

    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("cargo should set CARGO_MANIFEST_DIR");
    let mut fingerprint = Fingerprint::new();
    fingerprint
        .add_directory(&Path::new(&manifest_dir).join("src"))
        .expect("failed to read the crate sources");
    println!("cargo::rustc-env=WHISKER_TYPES_FINGERPRINT={fingerprint}");
}
