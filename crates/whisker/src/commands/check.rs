use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use clawless::prelude::*;
use whisker_core::Pipeline;
use whisker_rust::{RustDecorationProvider, RustLintPassAdapter};
use whisker_types::{DecorationProvider, Language, LintPass, Severity};

/// Run whisker lints against a project
#[derive(Debug, Args)]
pub struct CheckArgs {
    // r[impl cli.check.path]
    /// Path to the target project directory
    #[arg(default_value = ".")]
    path: PathBuf,

    // r[impl cli.check.keep-going]
    /// Continue checking after encountering errors
    #[arg(long)]
    keep_going: bool,

    // r[impl cli.check.extra-args]
    /// Additional arguments forwarded to the analysis pipeline
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

/// Builds the lint passes the CLI runs
///
/// Every rule ships as its own crate and is linked in here. A rule that is
/// not in this list does not run, however complete its own tests are.
// r[impl cli.check.passes]
fn create_lint_passes() -> Vec<Box<dyn LintPass>> {
    vec![
        Box::new(RustLintPassAdapter::new(
            anyhow_missing_context::AnyhowMissingContext,
        )),
        Box::new(RustLintPassAdapter::new(bool_param::BoolParam)),
        Box::new(RustLintPassAdapter::new(derive_order::DeriveOrder)),
        Box::new(RustLintPassAdapter::new(if_let_with_else::IfLetWithElse)),
        Box::new(RustLintPassAdapter::new(no_matches_macro::NoMatchesMacro)),
        Box::new(RustLintPassAdapter::new(
            wildcard_match_arm::WildcardMatchArm,
        )),
    ]
}

// r[impl cli.check]
#[command]
pub async fn check(args: CheckArgs, _context: Context) -> CommandResult {
    let CheckArgs {
        path,
        keep_going,
        args: _extra_args,
    } = args;

    let files = discover_files(&path)?;

    if files.is_empty() {
        return Ok(());
    }

    let mut pipeline =
        Pipeline::new(&whisker_rust::language()).context("failed to initialize pipeline")?;

    let provider = RustDecorationProvider::load(&path)
        .context("failed to load the target project for analysis")?;
    let providers: Vec<&dyn DecorationProvider> = vec![&provider];

    let mut all_diagnostics = Vec::new();
    let mut sources: HashMap<Arc<Path>, String> = HashMap::new();
    let mut had_error = false;

    for file in &files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                if keep_going {
                    eprintln!("error: {}: {e:#}", file.display());
                    had_error = true;
                    continue;
                }
                return Err(anyhow::anyhow!("read {}: {e}", file.display()).into());
            }
        };

        let mut passes = create_lint_passes();

        match pipeline.run_on_source(&source, file, &providers, &mut passes) {
            Ok(diagnostics) => {
                if !diagnostics.is_empty() {
                    let arc_path: Arc<Path> = file.clone().into();
                    sources.insert(arc_path, source);
                    all_diagnostics.extend(diagnostics);
                }
            }
            Err(e) => {
                if keep_going {
                    eprintln!("error: {}: {e:#}", file.display());
                    had_error = true;
                } else {
                    return Err(e.into());
                }
            }
        }
    }

    whisker_reporting::render_to_string(&all_diagnostics, &sources).and_then(|output| {
        if !output.is_empty() {
            eprint!("{output}");
        }
        Ok(())
    })?;

    // r[impl cli.diagnostics.exit-code]
    let has_errors = all_diagnostics
        .iter()
        .any(|d| d.severity() >= Severity::Error);

    if has_errors || had_error {
        std::process::exit(1);
    }

    Ok(())
}

fn discover_files(path: &PathBuf) -> anyhow::Result<Vec<PathBuf>> {
    anyhow::ensure!(path.exists(), "{} does not exist", path.display());

    if path.is_file() {
        return Ok(vec![path.clone()]);
    }

    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(ext) = entry.path().extension() else {
            continue;
        };
        if Language::from_extension(&ext.to_string_lossy()).is_some() {
            files.push(entry.into_path());
        }
    }

    Ok(files)
}
