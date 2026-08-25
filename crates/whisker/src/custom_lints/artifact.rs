use std::collections::BTreeMap;
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

/// Finds every dynamic library cargo built from a package under `directory`
///
/// `stdout` is the JSON message stream of a successful `cargo build`. A
/// configured entry may name a single package or a workspace of them, and
/// a repository of shared rules is usually the latter, so the search
/// collects all of them rather than one.
///
/// Artifacts are matched by where their manifest sits rather than by
/// package name, so a dependency that happens to share a plugin's name
/// cannot be loaded in its place. Dependencies resolve outside the
/// configured directory, into cargo's registry and git caches, which is
/// what makes containment a sufficient test.
///
/// The result is keyed by manifest path, which sorts it and keeps one
/// library per package in a single step. Both matter: cargo emits
/// artifacts in whatever order the build finished them, so lint order
/// would otherwise shift between runs, and a package cargo reports twice
/// would be loaded twice and report every finding twice.
///
/// # Errors
///
/// Returns an error if a message line is not valid JSON, or if the build
/// produced no dynamic library at all. The last error quotes the manifest
/// section to add, because a missing crate type is the most likely
/// authoring mistake.
pub fn cdylib_artifacts(stdout: &str, directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let directory = canonical(directory);
    let mut artifacts = BTreeMap::new();

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let message: BuildMessage = serde_json::from_str(line)
            .with_context(|| format!("failed to read a cargo build message: {line}"))?;

        if message.reason != "compiler-artifact" {
            continue;
        }

        let Some(manifest) = &message.manifest_path else {
            continue;
        };

        let manifest = canonical(manifest);
        if !manifest.starts_with(&directory) {
            continue;
        }

        let builds_cdylib = message
            .target
            .as_ref()
            .is_some_and(|target| target.crate_types.iter().any(|kind| kind == "cdylib"));
        if !builds_cdylib {
            continue;
        }

        let Some(library) = message.filenames.iter().find(|file| {
            file.extension()
                .is_some_and(|extension| extension == std::env::consts::DLL_EXTENSION)
        }) else {
            anyhow::bail!(
                "cargo reported no dynamic library file for {}",
                manifest.display()
            );
        };

        artifacts.insert(manifest, library.clone());
    }

    anyhow::ensure!(
        !artifacts.is_empty(),
        "cargo built no dynamic library from {}; a custom lint package needs\n\n[lib]\ncrate-type \
         = [\"cdylib\"]\n\nin its Cargo.toml",
        directory.display()
    );

    let artifacts = artifacts.into_values().collect();

    Ok(artifacts)
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

    const DIRECTORY: &str = "/plugin";
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
    fn cdylib_artifacts_finds_the_dynamic_library() {
        let stdout = [
            r#"{"reason":"compiler-artifact","manifest_path":"/deps/other/Cargo.toml","target":{"crate_types":["lib"],"name":"other"},"filenames":["/deps/libother.rlib"]}"#.to_string(),
            artifact_line(MANIFEST, r#""cdylib""#, &dylib_name("libplugin")),
            r#"{"reason":"build-finished","success":true}"#.to_string(),
        ]
        .join("\n");

        let artifacts = cdylib_artifacts(&stdout, Path::new(DIRECTORY)).expect("should find");

        assert_eq!(artifacts.len(), 1);
        assert!(artifacts[0].to_string_lossy().contains("libplugin"));
    }

    /// A dependency that builds a cdylib of its own resolves outside the
    /// configured directory, so containment keeps it out of the result.
    #[test]
    fn cdylib_artifacts_ignores_a_dependency_outside_the_directory() {
        let stdout = [
            r#"{"reason":"compiler-artifact","manifest_path":"/deps/other/Cargo.toml","target":{"crate_types":["cdylib"],"name":"other"},"filenames":["/deps/libother.so"]}"#.to_string(),
            artifact_line(MANIFEST, r#""cdylib""#, &dylib_name("libplugin")),
        ]
        .join("\n");

        let artifacts = cdylib_artifacts(&stdout, Path::new(DIRECTORY)).expect("should find");

        assert_eq!(artifacts.len(), 1);
        assert!(artifacts[0].to_string_lossy().contains("libplugin"));
    }

    #[test]
    fn cdylib_artifacts_ignores_unknown_message_reasons() {
        let stdout = [
            r#"{"reason":"a-future-cargo-message","novel_field":42}"#.to_string(),
            artifact_line(MANIFEST, r#""cdylib""#, &dylib_name("libplugin")),
        ]
        .join("\n");

        let artifacts = cdylib_artifacts(&stdout, Path::new(DIRECTORY));

        assert!(artifacts.is_ok());
    }

    /// A directory sharing a name prefix with the configured one is not
    /// inside it, and `starts_with` compares whole components.
    #[test]
    fn cdylib_artifacts_ignores_a_sibling_with_a_shared_prefix() {
        let stdout = [
            artifact_line(
                "/plugin_extra/Cargo.toml",
                r#""cdylib""#,
                "\"/plugin_extra/target/release/libextra.so\"",
            ),
            artifact_line(MANIFEST, r#""cdylib""#, &dylib_name("libplugin")),
        ]
        .join("\n");

        let artifacts = cdylib_artifacts(&stdout, Path::new(DIRECTORY)).expect("should find");

        assert_eq!(artifacts.len(), 1);
        assert!(artifacts[0].to_string_lossy().contains("libplugin"));
    }

    #[test]
    fn cdylib_artifacts_picks_the_platform_library_among_filenames() {
        let filenames = format!(
            "\"/plugin/target/release/libplugin.rlib\", {}",
            dylib_name("libplugin")
        );
        let stdout = artifact_line(MANIFEST, r#""cdylib", "rlib""#, &filenames);

        let artifacts = cdylib_artifacts(&stdout, Path::new(DIRECTORY)).expect("should find");

        assert!(
            artifacts[0]
                .extension()
                .is_some_and(|extension| extension == std::env::consts::DLL_EXTENSION)
        );
    }

    #[test]
    fn cdylib_artifacts_of_a_workspace_returns_every_member_sorted() {
        let stdout = [
            artifact_line(
                "/plugin/lints/second/Cargo.toml",
                r#""cdylib""#,
                &dylib_name("libsecond"),
            ),
            artifact_line(
                "/plugin/lints/first/Cargo.toml",
                r#""cdylib""#,
                &dylib_name("libfirst"),
            ),
        ]
        .join("\n");

        let artifacts = cdylib_artifacts(&stdout, Path::new(DIRECTORY)).expect("should find");

        assert_eq!(
            artifacts,
            vec![
                PathBuf::from(dylib_name("libfirst").trim_matches('"')),
                PathBuf::from(dylib_name("libsecond").trim_matches('"')),
            ],
            "members must load in a stable order, not in build-completion order"
        );
    }

    /// Cargo reports a package once per built target, and loading the same
    /// library twice would double every diagnostic it produces.
    #[test]
    fn cdylib_artifacts_reports_a_package_once() {
        let stdout = [
            artifact_line(MANIFEST, r#""cdylib""#, &dylib_name("libplugin")),
            artifact_line(MANIFEST, r#""cdylib""#, &dylib_name("libplugin")),
        ]
        .join("\n");

        let artifacts = cdylib_artifacts(&stdout, Path::new(DIRECTORY)).expect("should find");

        assert_eq!(artifacts.len(), 1);
    }

    #[test]
    fn cdylib_artifacts_with_malformed_line_returns_error() {
        let stdout = "this is not json\n";

        let error = cdylib_artifacts(stdout, Path::new(DIRECTORY)).expect_err("should fail");

        assert!(
            error.to_string().contains("cargo build message"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn cdylib_artifacts_without_cdylib_crate_type_names_the_remedy() {
        let stdout = artifact_line(
            MANIFEST,
            r#""lib""#,
            "\"/plugin/target/release/libplugin.rlib\"",
        );

        let error = cdylib_artifacts(&stdout, Path::new(DIRECTORY)).expect_err("should fail");

        assert!(
            format!("{error:#}").contains("crate-type = [\"cdylib\"]"),
            "the error should quote the manifest fix: {error:#}"
        );
    }

    #[test]
    fn cdylib_artifacts_without_package_artifact_returns_error() {
        let stdout = r#"{"reason":"compiler-artifact","manifest_path":"/deps/other/Cargo.toml","target":{"crate_types":["cdylib"],"name":"other"},"filenames":["/deps/libother.so"]}"#;

        let error = cdylib_artifacts(stdout, Path::new(DIRECTORY)).expect_err("should fail");

        assert!(
            error.to_string().contains("no dynamic library"),
            "unexpected: {error:#}"
        );
    }
}
