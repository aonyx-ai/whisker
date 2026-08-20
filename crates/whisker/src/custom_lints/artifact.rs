use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Deserialize;

/// One line of `cargo build --message-format=json-render-diagnostics`
///
/// Only the fields the artifact search reads are declared, and only
/// `reason` is required, because cargo adds message kinds and fields over
/// time and an unknown one is not an error.
#[derive(Debug, Deserialize)]
struct BuildMessage {
    reason: String,
    #[serde(default)]
    manifest_path: Option<PathBuf>,
    #[serde(default)]
    target: Option<Target>,
    #[serde(default)]
    filenames: Vec<PathBuf>,
}

/// The target a compiler artifact was built from
#[derive(Debug, Deserialize)]
struct Target {
    #[serde(default)]
    crate_types: Vec<String>,
}

/// Finds the dynamic library that cargo built for the package at `manifest`
///
/// `stdout` is the JSON message stream of a successful `cargo build`. The
/// search matches artifacts by manifest path rather than by package name,
/// so a dependency that happens to share the plugin's name cannot be
/// loaded in its place.
///
/// # Errors
///
/// Returns an error if a message line is not valid JSON, if the build
/// produced no artifact for `manifest`, or if the package does not build a
/// `cdylib`. The last error quotes the manifest section to add, because a
/// missing crate type is the most likely authoring mistake.
pub fn cdylib_artifact(stdout: &str, manifest: &Path) -> anyhow::Result<PathBuf> {
    let manifest = canonical(manifest);
    let mut package_artifacts = Vec::new();

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let message: BuildMessage = serde_json::from_str(line)
            .with_context(|| format!("failed to read a cargo build message: {line}"))?;

        if message.reason != "compiler-artifact" {
            continue;
        }

        let Some(path) = &message.manifest_path else {
            continue;
        };

        if canonical(path) == manifest {
            package_artifacts.push(message);
        }
    }

    anyhow::ensure!(
        !package_artifacts.is_empty(),
        "cargo built no artifact for {}; the configured path must name a single cargo package, \
         not a workspace",
        manifest.display()
    );

    let cdylib = package_artifacts
        .into_iter()
        .rev()
        .find(|artifact| {
            artifact
                .target
                .as_ref()
                .is_some_and(|target| target.crate_types.iter().any(|kind| kind == "cdylib"))
        })
        .with_context(|| {
            format!(
                "the package at {} does not build a dynamic library; add\n\n[lib]\ncrate-type = \
                 [\"cdylib\"]\n\nto its Cargo.toml",
                manifest.display()
            )
        })?;

    cdylib
        .filenames
        .iter()
        .find(|file| {
            file.extension()
                .is_some_and(|extension| extension == std::env::consts::DLL_EXTENSION)
        })
        .cloned()
        .with_context(|| {
            format!(
                "cargo reported no dynamic library file for {}",
                manifest.display()
            )
        })
}

/// Resolves a path for comparison, or leaves it as written
///
/// Cargo prints absolute manifest paths, but whether symlinks are resolved
/// is its business; resolving both sides makes the comparison meet in the
/// middle. The fallback keeps the function total, so it stays testable on
/// paths that do not exist.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "/plugin/Cargo.toml";

    fn artifact_line(manifest: &str, crate_types: &str, filenames: &str) -> String {
        format!(
            r#"{{"reason":"compiler-artifact","manifest_path":"{manifest}","target":{{"crate_types":[{crate_types}],"name":"plugin"}},"filenames":[{filenames}]}}"#
        )
    }

    fn dylib_name(stem: &str) -> String {
        format!(
            "\"/plugin/target/release/{stem}.{}\"",
            std::env::consts::DLL_EXTENSION
        )
    }

    #[test]
    fn cdylib_artifact_finds_the_dynamic_library() {
        let stdout = [
            r#"{"reason":"compiler-artifact","manifest_path":"/deps/other/Cargo.toml","target":{"crate_types":["lib"],"name":"other"},"filenames":["/deps/libother.rlib"]}"#.to_string(),
            artifact_line(MANIFEST, r#""cdylib""#, &dylib_name("libplugin")),
            r#"{"reason":"build-finished","success":true}"#.to_string(),
        ]
        .join("\n");

        let artifact = cdylib_artifact(&stdout, Path::new(MANIFEST)).expect("should find");

        assert!(artifact.to_string_lossy().contains("libplugin"));
    }

    #[test]
    fn cdylib_artifact_ignores_unknown_message_reasons() {
        let stdout = [
            r#"{"reason":"a-future-cargo-message","novel_field":42}"#.to_string(),
            artifact_line(MANIFEST, r#""cdylib""#, &dylib_name("libplugin")),
        ]
        .join("\n");

        let artifact = cdylib_artifact(&stdout, Path::new(MANIFEST));

        assert!(artifact.is_ok());
    }

    #[test]
    fn cdylib_artifact_picks_the_platform_library_among_filenames() {
        let filenames = format!(
            "\"/plugin/target/release/libplugin.rlib\", {}",
            dylib_name("libplugin")
        );
        let stdout = artifact_line(MANIFEST, r#""cdylib", "rlib""#, &filenames);

        let artifact = cdylib_artifact(&stdout, Path::new(MANIFEST)).expect("should find");

        assert!(
            artifact
                .extension()
                .is_some_and(|extension| extension == std::env::consts::DLL_EXTENSION)
        );
    }

    #[test]
    fn cdylib_artifact_with_malformed_line_returns_error() {
        let stdout = "this is not json\n";

        let error = cdylib_artifact(stdout, Path::new(MANIFEST)).expect_err("should fail");

        assert!(
            error.to_string().contains("cargo build message"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn cdylib_artifact_without_cdylib_crate_type_names_the_remedy() {
        let stdout = artifact_line(
            MANIFEST,
            r#""lib""#,
            "\"/plugin/target/release/libplugin.rlib\"",
        );

        let error = cdylib_artifact(&stdout, Path::new(MANIFEST)).expect_err("should fail");

        assert!(
            format!("{error:#}").contains("crate-type = [\"cdylib\"]"),
            "the error should quote the manifest fix: {error:#}"
        );
    }

    #[test]
    fn cdylib_artifact_without_package_artifact_returns_error() {
        let stdout = r#"{"reason":"compiler-artifact","manifest_path":"/deps/other/Cargo.toml","target":{"crate_types":["cdylib"],"name":"other"},"filenames":["/deps/libother.so"]}"#;

        let error = cdylib_artifact(stdout, Path::new(MANIFEST)).expect_err("should fail");

        assert!(
            error.to_string().contains("no artifact"),
            "unexpected: {error:#}"
        );
    }
}
