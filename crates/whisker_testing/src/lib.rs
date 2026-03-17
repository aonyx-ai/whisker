/// Runs dylint lint tests with both stderr diffing and annotation checking
///
/// This runs `compiletest_rs` twice against the same test fixtures:
///
/// 1. **UI mode** — diffs actual stderr against `.stderr` snapshot files
/// 2. **compile-fail mode** — checks `//~` annotations in the source
///    against actual compiler output (uses `-D warnings` to promote
///    warnings to errors so compilation fails as expected)
///
/// The `//~` annotations use the same syntax as `rustc`'s own test
/// suite:
///
/// - `//~ ERROR message` — expects an error on this line
/// - `//~^ ERROR message` — expects an error on the previous line
/// - `//~| ERROR message` — expects an error on the same line as above
///
/// Note: use `ERROR` (not `WARN`) in annotations because `-D warnings`
/// promotes all warnings to errors.
///
/// # Panics
///
/// Panics if either check fails.
pub fn ui_test(name: &str, src_base: &str) {
    let driver = initialize(name);
    let src_base = std::path::Path::new(src_base);

    // r[impl testing.ui-mode]
    run_compiletest(driver, src_base, compiletest_rs::common::Mode::Ui, "");
    // r[impl testing.annotation-mode]
    if has_annotations(src_base) {
        run_compiletest(
            driver,
            src_base,
            compiletest_rs::common::Mode::CompileFail,
            "-D warnings",
        );
    }
}

use std::path::{Path, PathBuf};

use dylint_internal::CommandExt;
use once_cell::sync::OnceCell;

static DRIVER: OnceCell<PathBuf> = OnceCell::new();

// r[impl testing.driver-setup]
fn initialize(name: &str) -> &'static Path {
    DRIVER
        .get_or_init(|| {
            let _ = env_logger::try_init();

            dylint_internal::cargo::build(&format!("library `{name}`"))
                .build()
                .success()
                .expect("failed to build lint library");

            let metadata =
                dylint_internal::cargo::current_metadata().expect("failed to get cargo metadata");
            let dylint_library_path = metadata.target_directory.join("debug");

            unsafe {
                std::env::set_var(
                    dylint_internal::env::DYLINT_LIBRARY_PATH,
                    &dylint_library_path,
                );
            }

            let dylint_libs = dylint_libs(name, dylint_library_path.as_std_path());

            let driver = dylint::driver_builder::get(
                &dylint::opts::Dylint::default(),
                env!("RUSTUP_TOOLCHAIN"),
            )
            .expect("failed to find dylint driver");

            unsafe {
                std::env::set_var(dylint_internal::env::CLIPPY_DISABLE_DOCS_LINKS, "true");
                std::env::set_var(dylint_internal::env::DYLINT_LIBS, &dylint_libs);
            }

            driver
        })
        .as_path()
}

fn dylint_libs(name: &str, target_dir: &Path) -> String {
    let rustup_toolchain = dylint_internal::env::var(dylint_internal::env::RUSTUP_TOOLCHAIN)
        .expect("RUSTUP_TOOLCHAIN not set");
    let filename = dylint_internal::library_filename(name, &rustup_toolchain);
    let path = target_dir.join(filename);
    serde_json::to_string(&vec![path]).expect("failed to serialize library paths")
}

fn has_annotations(src_base: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(src_base) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        path.extension().is_some_and(|ext| ext == "rs")
            && std::fs::read_to_string(&path).is_ok_and(|content| content.contains("//~"))
    })
}

fn run_compiletest(
    driver: &Path,
    src_base: &Path,
    mode: compiletest_rs::common::Mode,
    extra_flags: &str,
) {
    let mut flags = "--emit=metadata -Zui-testing".to_string();
    if !extra_flags.is_empty() {
        flags.push(' ');
        flags.push_str(extra_flags);
    }

    let compile_test_exit_code = match mode {
        compiletest_rs::common::Mode::CompileFail => Some(101),
        compiletest_rs::common::Mode::ParseFail => Some(101),
        compiletest_rs::common::Mode::Ui
        | compiletest_rs::common::Mode::RunFail
        | compiletest_rs::common::Mode::RunPass
        | compiletest_rs::common::Mode::RunPassValgrind
        | compiletest_rs::common::Mode::Pretty
        | compiletest_rs::common::Mode::DebugInfoGdb
        | compiletest_rs::common::Mode::DebugInfoLldb
        | compiletest_rs::common::Mode::Codegen
        | compiletest_rs::common::Mode::Rustdoc
        | compiletest_rs::common::Mode::CodegenUnits
        | compiletest_rs::common::Mode::Incremental
        | compiletest_rs::common::Mode::RunMake
        | compiletest_rs::common::Mode::MirOpt
        | compiletest_rs::common::Mode::Assembly => None,
    };

    let config = compiletest_rs::Config {
        mode,
        rustc_path: driver.to_path_buf(),
        src_base: src_base.to_path_buf(),
        target_rustcflags: Some(flags),
        compile_test_exit_code,
        ..compiletest_rs::Config::default()
    };

    compiletest_rs::run_tests(&config);
}
