mod check_outcome;
mod error_recovery;
mod failure_threshold;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use clawless::prelude::*;
use whisker_core::Pipeline;
use whisker_rust::{RustDecorationProvider, RustLintPassAdapter};
use whisker_types::{DecorationProvider, Diagnostic, LintPass};

use self::check_outcome::CheckOutcome;
use self::error_recovery::ErrorRecovery;
use self::failure_threshold::FailureThreshold;
use crate::config::WhiskerConfig;
use crate::discovery::{Discovery, WalkErrorPolicy};

/// Run whisker lints against a project
#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Path to the target project directory
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Continue checking after encountering errors
    #[arg(long)]
    keep_going: bool,

    /// Treat warnings as errors
    #[arg(long)]
    deny_warnings: bool,

    /// Additional arguments forwarded to the analysis pipeline
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

/// Builds the lint passes the CLI runs
///
/// Every rule ships as its own crate and is linked in here. A rule that is
/// not in this list does not run, however complete its own tests are.
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

#[command]
pub async fn check(args: CheckArgs, _context: Context) -> CommandResult {
    let CheckArgs {
        path,
        keep_going,
        deny_warnings,
        args: _extra_args,
    } = args;

    let threshold = match deny_warnings {
        true => FailureThreshold::Warnings,
        false => FailureThreshold::Errors,
    };

    let recovery = match keep_going {
        true => ErrorRecovery::Continue,
        false => ErrorRecovery::Abort,
    };

    anyhow::ensure!(path.exists(), "{} does not exist", path.display());

    let config = WhiskerConfig::load(&path).context("failed to load the whisker configuration")?;

    let on_error = match keep_going {
        true => WalkErrorPolicy::ReportAndContinue,
        false => WalkErrorPolicy::Fail,
    };

    let discovery =
        Discovery::run(&path, &config, on_error).context("failed to discover source files")?;

    let mut outcome = CheckOutcome::Success;

    for error in discovery.errors() {
        eprintln!("error: {error:#}");
        outcome = CheckOutcome::Failure;
    }

    let files = discovery.files();

    anyhow::ensure!(
        !files.is_empty(),
        "whisker analyzed no files under {}: either nothing there is written in a language \
         whisker has a grammar for, or the ignore files and configured patterns excluded all of \
         it",
        path.display()
    );

    let mut pipeline =
        Pipeline::new(&whisker_rust::language()).context("failed to initialize pipeline")?;

    let provider = RustDecorationProvider::load(&path)
        .context("failed to load the target project for analysis")?;
    let providers: Vec<&dyn DecorationProvider> = vec![&provider];

    let mut all_diagnostics = Vec::new();
    let mut sources: HashMap<Arc<Path>, String> = HashMap::new();

    for file in files {
        let source = match std::fs::read_to_string(file) {
            Ok(source) => source,
            Err(e) => {
                recovery.record(&mut outcome, file, anyhow::Error::new(e))?;
                continue;
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
            Err(e) => recovery.record(&mut outcome, file, e)?,
        }
    }

    let all_diagnostics: Vec<Diagnostic> = all_diagnostics
        .into_iter()
        .map(|diagnostic| threshold.promote(diagnostic))
        .collect();

    let output = whisker_reporting::render_to_string(&all_diagnostics, &sources)?;
    if !output.is_empty() {
        eprint!("{output}");
    }

    let outcome = outcome.combine(CheckOutcome::from_diagnostics(&all_diagnostics, threshold));

    match outcome {
        CheckOutcome::Failure => std::process::exit(1),
        CheckOutcome::Success => Ok(()),
    }
}
