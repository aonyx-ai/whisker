/// Bakes the platform this binary was built for into it
///
/// Cargo knows the target triple and the binary does not. Whisker needs
/// it to ask a publisher for a prebuilt lint library that runs here.
fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let target = std::env::var("TARGET").expect("cargo sets TARGET for a build script");
    println!("cargo::rustc-env=WHISKER_TARGET={target}");
}
