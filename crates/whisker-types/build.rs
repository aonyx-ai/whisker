/// Bakes the compiler's identity into the crate
///
/// Rust has no stable ABI, so whisker only loads a plugin its own rustc
/// compiled. The layout of the boundary is fingerprinted in the crate
/// itself rather than here, because a build script sees source text and
/// the handshake cares about how that text lays out in memory.
fn main() {
    let version = rustc_version::version_meta().expect("failed to read the rustc version");
    println!(
        "cargo::rustc-env=WHISKER_RUSTC_VERSION={}",
        version.short_version_string
    );
}
