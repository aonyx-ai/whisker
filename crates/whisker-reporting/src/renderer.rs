use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use codespan_reporting::diagnostic as codespan_diag;
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use whisker_types::{Diagnostic, Severity};

/// Renders diagnostics to stderr using codespan-reporting
///
/// Groups diagnostics by file, loads each file's source once, and renders
/// all diagnostics with primary spans, origin annotations, related
/// annotations, and suggestion notes.
///
/// # Errors
///
/// Returns an error if a source file cannot be read or if writing to
/// stderr fails.
// r[impl reporting.output]
pub fn render(diagnostics: &[Diagnostic]) -> anyhow::Result<()> {
    if diagnostics.is_empty() {
        return Ok(());
    }

    let writer = StandardStream::stderr(ColorChoice::Auto);
    let config = term::Config::default();

    let sources = collect_sources(diagnostics)?;

    let mut files = SimpleFiles::new();
    let mut file_ids: HashMap<Arc<Path>, usize> = HashMap::new();

    for (path, source) in &sources {
        let id = files.add(path.display().to_string(), source.as_str());
        file_ids.insert(path.clone(), id);
    }

    let mut writer = writer.lock();

    for diag in diagnostics {
        let codespan_diag = to_codespan(diag, &file_ids);
        term::emit_to_io_write(&mut writer, &config, &files, &codespan_diag)?;
        writer.flush()?;
    }

    Ok(())
}

fn to_codespan(
    diag: &Diagnostic,
    file_ids: &HashMap<Arc<Path>, usize>,
) -> codespan_diag::Diagnostic<usize> {
    let severity = match diag.severity() {
        Severity::Error => codespan_diag::Severity::Error,
        Severity::Warn => codespan_diag::Severity::Warning,
        Severity::Info => codespan_diag::Severity::Note,
        Severity::Help => codespan_diag::Severity::Help,
    };

    let mut labels = Vec::new();

    if let Some(&file_id) = file_ids.get(diag.span().file()) {
        labels.push(
            codespan_diag::Label::primary(file_id, diag.span().start()..diag.span().end())
                .with_message(diag.message()),
        );
    }

    // r[impl reporting.origins]
    for origin in diag.origins() {
        if let Some(&file_id) = file_ids.get(origin.span().file()) {
            labels.push(
                codespan_diag::Label::secondary(
                    file_id,
                    origin.span().start()..origin.span().end(),
                )
                .with_message(origin.message()),
            );
        }
    }

    // r[impl reporting.related]
    for related in diag.related() {
        if let Some(&file_id) = file_ids.get(related.span().file()) {
            labels.push(
                codespan_diag::Label::secondary(
                    file_id,
                    related.span().start()..related.span().end(),
                )
                .with_message(related.message()),
            );
        }
    }

    let mut notes = Vec::new();

    // r[impl reporting.suggestions]
    for suggestion in diag.suggestions() {
        notes.push(format!(
            "{}: replace with `{}`",
            suggestion.message(),
            suggestion.replacement()
        ));
    }

    codespan_diag::Diagnostic::new(severity)
        .with_code(diag.rule_id().to_string())
        .with_message(diag.message())
        .with_labels(labels)
        .with_notes(notes)
}

/// Renders diagnostics to a string, given pre-loaded source files
///
/// This is the testable core of the rendering pipeline. The caller
/// provides a map of file paths to source contents, avoiding filesystem
/// access.
///
/// # Errors
///
/// Returns an error if codespan-reporting fails to render a diagnostic
/// or if the rendered output is not valid UTF-8.
pub fn render_to_string(
    diagnostics: &[Diagnostic],
    sources: &HashMap<Arc<Path>, String>,
) -> anyhow::Result<String> {
    if diagnostics.is_empty() {
        return Ok(String::new());
    }

    let mut files = SimpleFiles::new();
    let mut file_ids: HashMap<Arc<Path>, usize> = HashMap::new();

    for (path, source) in sources {
        let id = files.add(path.display().to_string(), source.as_str());
        file_ids.insert(path.clone(), id);
    }

    let config = term::Config::default();
    let mut output = Vec::new();

    for diag in diagnostics {
        let codespan_diag = to_codespan(diag, &file_ids);
        term::emit_to_io_write(&mut output, &config, &files, &codespan_diag)?;
    }

    Ok(String::from_utf8(output)?)
}

fn collect_sources(diagnostics: &[Diagnostic]) -> anyhow::Result<HashMap<Arc<Path>, String>> {
    let mut sources = HashMap::new();

    for diag in diagnostics {
        let file = Arc::clone(diag.span().file_arc());
        if let std::collections::hash_map::Entry::Vacant(entry) = sources.entry(file) {
            let source = std::fs::read_to_string(entry.key())
                .map_err(|e| anyhow::anyhow!("read {}: {e}", entry.key().display()))?;
            entry.insert(source);
        }
    }

    Ok(sources)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use whisker_types::{Location, RuleId, Span, Suggestion};

    use super::*;

    fn test_source() -> HashMap<Arc<Path>, String> {
        let mut sources = HashMap::new();
        let file: Arc<Path> = PathBuf::from("test.rs").into();
        sources.insert(file, "fn main() {}\n".into());
        sources
    }

    fn test_diagnostic() -> Diagnostic {
        Diagnostic::new(
            RuleId("lint.test"),
            Severity::Warn,
            "test warning".into(),
            Span::new(PathBuf::from("test.rs"), 0, 12),
        )
    }

    #[test]
    fn render_to_string_with_empty_diagnostics_returns_empty() {
        let result = render_to_string(&[], &HashMap::new()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn render_to_string_includes_rule_id() {
        let output = render_to_string(&[test_diagnostic()], &test_source()).unwrap();
        assert!(
            output.contains("lint.test"),
            "output should contain rule id: {output}"
        );
    }

    #[test]
    fn render_to_string_includes_message() {
        let output = render_to_string(&[test_diagnostic()], &test_source()).unwrap();
        assert!(
            output.contains("test warning"),
            "output should contain message: {output}"
        );
    }

    #[test]
    fn render_to_string_shows_warning_severity() {
        let output = render_to_string(&[test_diagnostic()], &test_source()).unwrap();
        assert!(
            output.contains("warning"),
            "output should contain 'warning': {output}"
        );
    }

    #[test]
    fn render_to_string_shows_error_severity() {
        let diag = Diagnostic::new(
            RuleId("lint.err"),
            Severity::Error,
            "test error".into(),
            Span::new(PathBuf::from("test.rs"), 0, 12),
        );
        let output = render_to_string(&[diag], &test_source()).unwrap();
        assert!(
            output.contains("error"),
            "output should contain 'error': {output}"
        );
    }

    #[test]
    fn render_to_string_includes_origin_annotation() {
        let diag = test_diagnostic().with_origin(Location::new(
            Span::new(PathBuf::from("test.rs"), 3, 7),
            "defined here".into(),
        ));
        let output = render_to_string(&[diag], &test_source()).unwrap();
        assert!(
            output.contains("defined here"),
            "output should contain origin message: {output}"
        );
    }

    #[test]
    fn render_to_string_includes_related_annotation() {
        let diag = test_diagnostic().with_related(Location::new(
            Span::new(PathBuf::from("test.rs"), 3, 7),
            "also here".into(),
        ));
        let output = render_to_string(&[diag], &test_source()).unwrap();
        assert!(
            output.contains("also here"),
            "output should contain related message: {output}"
        );
    }

    #[test]
    fn render_to_string_includes_suggestion_note() {
        let diag = test_diagnostic().with_suggestion(Suggestion::new(
            Span::new(PathBuf::from("test.rs"), 0, 2),
            "pub fn".into(),
            "make it public".into(),
        ));
        let output = render_to_string(&[diag], &test_source()).unwrap();
        assert!(
            output.contains("make it public"),
            "output should contain suggestion message: {output}"
        );
        assert!(
            output.contains("pub fn"),
            "output should contain replacement: {output}"
        );
    }

    #[test]
    fn render_to_string_renders_multiple_diagnostics() {
        let d1 = Diagnostic::new(
            RuleId("lint.a"),
            Severity::Warn,
            "first".into(),
            Span::new(PathBuf::from("test.rs"), 0, 2),
        );
        let d2 = Diagnostic::new(
            RuleId("lint.b"),
            Severity::Error,
            "second".into(),
            Span::new(PathBuf::from("test.rs"), 3, 7),
        );
        let output = render_to_string(&[d1, d2], &test_source()).unwrap();
        assert!(
            output.contains("first"),
            "should contain first diag: {output}"
        );
        assert!(
            output.contains("second"),
            "should contain second diag: {output}"
        );
    }

    #[test]
    fn render_to_string_shows_source_file_name() {
        let output = render_to_string(&[test_diagnostic()], &test_source()).unwrap();
        assert!(
            output.contains("test.rs"),
            "output should contain filename: {output}"
        );
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        fn arb_severity() -> impl Strategy<Value = Severity> {
            prop_oneof![
                Just(Severity::Help),
                Just(Severity::Info),
                Just(Severity::Warn),
                Just(Severity::Error),
            ]
        }

        proptest! {
            #[test]
            fn render_to_string_always_contains_message(
                message in "[a-z ]{1,30}",
                severity in arb_severity(),
            ) {
                let diag = Diagnostic::new(
                    RuleId("lint.prop"),
                    severity,
                    message.clone(),
                    Span::new(PathBuf::from("test.rs"), 0, 5),
                );
                let output = render_to_string(
                    &[diag],
                    &test_source(),
                ).unwrap();
                prop_assert!(output.contains(&message));
            }

            #[test]
            fn render_to_string_always_contains_rule_id(
                severity in arb_severity(),
            ) {
                let diag = Diagnostic::new(
                    RuleId("lint.propcheck"),
                    severity,
                    "msg".into(),
                    Span::new(PathBuf::from("test.rs"), 0, 5),
                );
                let output = render_to_string(
                    &[diag],
                    &test_source(),
                ).unwrap();
                prop_assert!(output.contains("lint.propcheck"));
            }

            #[test]
            fn render_to_string_multiple_diagnostics_all_appear(
                count in 1..=5usize,
            ) {
                let diagnostics: Vec<Diagnostic> = (0..count)
                    .map(|i| {
                        Diagnostic::new(
                            RuleId("lint.multi"),
                            Severity::Warn,
                            format!("diag_{i}"),
                            Span::new(PathBuf::from("test.rs"), 0, 5),
                        )
                    })
                    .collect();
                let output = render_to_string(
                    &diagnostics,
                    &test_source(),
                ).unwrap();
                for i in 0..count {
                    let needle = format!("diag_{i}");
                    prop_assert!(output.contains(&needle));
                }
            }
        }
    }
}
