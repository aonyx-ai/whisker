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
use whisker_types::{DecorationProvider, Diagnostic, LintPass, UncoveredFile};

use self::check_outcome::CheckOutcome;
use self::error_recovery::ErrorRecovery;
use self::failure_threshold::FailureThreshold;
use crate::config::WhiskerConfig;
use crate::discovery::{Discovery, WalkErrorPolicy};

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

    // r[impl cli.check.deny-warnings]
    /// Treat warnings as errors
    #[arg(long)]
    deny_warnings: bool,

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

/// The distinct remedies for the files this run could not analyze
///
/// Remedies are gathered for the whole run and printed once, because a
/// directory of orphaned files would otherwise repeat the same sentence on
/// every line and bury the diagnostics. Each one comes from the
/// [`CoverageGap`] that produced it, so a run that trips two kinds of gap
/// offers both fixes rather than one that fits neither.
///
/// [`CoverageGap`]: whisker_types::CoverageGap
#[derive(Clone, Eq, PartialEq, Debug, Default)]
struct CoverageRemedies {
    remedies: Vec<&'static str>,
}

impl CoverageRemedies {
    /// Records the remedy for every gap reported against one file
    ///
    /// Repeats are dropped, so the number of lines this eventually prints
    /// is bounded by the number of [`CoverageGap`] variants however many
    /// files the run skipped.
    ///
    /// [`CoverageGap`]: whisker_types::CoverageGap
    fn record(&mut self, uncovered: &UncoveredFile) {
        for (_provider, gap) in uncovered.gaps() {
            let remedy = gap.help();
            if !self.remedies.contains(&remedy) {
                self.remedies.push(remedy);
            }
        }
    }

    /// Prints each remedy once, after the per-file errors that earned it
    ///
    /// The per-file error names the file and the reason; this names the fix.
    fn print(&self) {
        for remedy in &self.remedies {
            eprintln!("help: {remedy}");
        }
    }
}

// r[impl cli.check]
// r[impl cli.check.coverage]
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

    // r[impl cli.discovery.walk-errors]
    for error in discovery.errors() {
        eprintln!("error: {error:#}");
        outcome = CheckOutcome::Failure;
    }

    let files = discovery.files();

    // r[impl cli.discovery.empty]
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
    let mut remedies = CoverageRemedies::default();

    for file in files {
        let source = match std::fs::read_to_string(file) {
            Ok(source) => source,
            Err(e) => {
                if let Err(aborted) = recovery.record(&mut outcome, file, anyhow::Error::new(e)) {
                    remedies.print();
                    return Err(aborted);
                }
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
            Err(e) => {
                if let Some(uncovered) = e.downcast_ref::<UncoveredFile>() {
                    remedies.record(uncovered);
                }

                if let Err(aborted) = recovery.record(&mut outcome, file, e) {
                    remedies.print();
                    return Err(aborted);
                }
            }
        }
    }

    remedies.print();

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

#[cfg(test)]
mod tests {
    use whisker_types::{CoverageGap, ProviderName};

    use super::*;

    fn root() -> Arc<Path> {
        Arc::from(PathBuf::from("/ws"))
    }

    #[test]
    fn record_with_no_gaps_collects_no_remedy() {
        let uncovered = UncoveredFile::new(PathBuf::from("stray.rs"), Vec::new());
        let mut remedies = CoverageRemedies::default();

        remedies.record(&uncovered);

        assert!(remedies.remedies.is_empty());
    }

    #[test]
    fn record_with_repeated_gap_keeps_one_remedy() {
        let gap = CoverageGap::Unreachable { root: root() };
        let first = UncoveredFile::new(
            PathBuf::from("one.rs"),
            vec![(ProviderName("rust"), gap.clone())],
        );
        let second = UncoveredFile::new(PathBuf::from("two.rs"), vec![(ProviderName("rust"), gap)]);
        let mut remedies = CoverageRemedies::default();

        remedies.record(&first);
        remedies.record(&second);

        assert_eq!(remedies.remedies.len(), 1);
        assert_eq!(
            remedies.remedies[0],
            CoverageGap::Unreachable { root: root() }.help()
        );
    }

    #[test]
    fn record_with_two_kinds_of_gap_keeps_both_remedies() {
        let uncovered = UncoveredFile::new(
            PathBuf::from("stray.rs"),
            vec![
                (
                    ProviderName("rust"),
                    CoverageGap::Unreachable { root: root() },
                ),
                (ProviderName("other"), CoverageGap::StaleSource),
            ],
        );
        let mut remedies = CoverageRemedies::default();

        remedies.record(&uncovered);

        assert_eq!(remedies.remedies.len(), 2);
        assert_eq!(remedies.remedies[1], CoverageGap::StaleSource.help());
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CoverageRemedies>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CoverageRemedies>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CoverageRemedies>();
    }
}
