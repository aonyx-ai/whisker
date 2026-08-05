use std::path::Path;
use std::sync::Arc;

use crate::{CoverageGap, ProviderName};

/// Error returned when no decoration provider claims a file
///
/// The error can travel inside an [`anyhow::Error`], and a caller can
/// downcast it to report each provider's reason in detail.
///
/// # Examples
///
/// ```
/// use std::path::{Path, PathBuf};
/// use std::sync::Arc;
///
/// use whisker_types::{CoverageGap, ProviderName, UncoveredFile};
///
/// let root: Arc<Path> = Arc::from(PathBuf::from("/ws"));
/// let error = UncoveredFile::new(
///     PathBuf::from("stray.rs"),
///     vec![(ProviderName("rust"), CoverageGap::Unreachable { root })],
/// );
///
/// assert_eq!(
///     error.to_string(),
///     "no decoration provider covers this file, so semantic rules cannot run\n  \
///      rust: the file is inside /ws, but nothing the toolchain loaded reaches it",
/// );
/// ```
///
/// [`anyhow::Error`]: anyhow::Error
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct UncoveredFile {
    file: Arc<Path>,
    gaps: Vec<(ProviderName, CoverageGap)>,
}

impl UncoveredFile {
    /// Records that no provider claimed `file`, with each provider's reason
    ///
    /// An empty `gaps` means no providers were consulted, and the error
    /// renders its own message for that case.
    pub fn new(file: impl Into<Arc<Path>>, gaps: Vec<(ProviderName, CoverageGap)>) -> Self {
        Self {
            file: file.into(),
            gaps,
        }
    }

    /// Returns the file that could not be analyzed
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns each provider consulted and its reason for declining
    ///
    /// The slice is empty when no providers were configured.
    pub fn gaps(&self) -> &[(ProviderName, CoverageGap)] {
        &self.gaps
    }
}

impl std::fmt::Display for UncoveredFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.gaps.is_empty() {
            return f
                .write_str("no decoration providers were configured, so no file can be analyzed");
        }

        f.write_str("no decoration provider covers this file, so semantic rules cannot run")?;

        for (name, gap) in &self.gaps {
            write!(f, "\n  {name}: {gap}")?;
        }

        Ok(())
    }
}

impl std::error::Error for UncoveredFile {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn root() -> Arc<Path> {
        Arc::from(PathBuf::from("/ws"))
    }

    #[test]
    fn display_with_no_gaps_reports_no_providers_configured() {
        let error = UncoveredFile::new(PathBuf::from("stray.rs"), Vec::new());

        let message = error.to_string();

        assert_eq!(
            message,
            "no decoration providers were configured, so no file can be analyzed"
        );
    }

    #[test]
    fn display_with_one_gap_names_provider_and_reason() {
        let error = UncoveredFile::new(
            PathBuf::from("stray.rs"),
            vec![(
                ProviderName("rust"),
                CoverageGap::Unreachable { root: root() },
            )],
        );

        let message = error.to_string();

        assert_eq!(
            message,
            "no decoration provider covers this file, so semantic rules cannot run\n  \
             rust: the file is inside /ws, but nothing the toolchain loaded reaches it"
        );
    }

    #[test]
    fn display_with_two_gaps_lists_both() {
        let error = UncoveredFile::new(
            PathBuf::from("stray.rs"),
            vec![
                (ProviderName("rust"), CoverageGap::StaleSource),
                (
                    ProviderName("other"),
                    CoverageGap::OutsideRoot { root: root() },
                ),
            ],
        );

        let message = error.to_string();

        let lines: Vec<&str> = message.lines().collect();
        assert_eq!(lines.len(), 3, "unexpected message: {message}");
        assert_eq!(
            lines[1],
            "  rust: the text whisker parsed differs from the text the toolchain analyzed"
        );
        assert_eq!(
            lines[2],
            "  other: the file is outside /ws, the root the provider loaded"
        );
        assert!(!message.ends_with('\n'), "message has a trailing newline");
    }

    #[test]
    fn file_returns_path() {
        let error = UncoveredFile::new(PathBuf::from("a/stray.rs"), Vec::new());

        let file = error.file();

        assert_eq!(file, Path::new("a/stray.rs"));
    }

    #[test]
    fn gaps_returns_every_provider_consulted() {
        let error = UncoveredFile::new(
            PathBuf::from("stray.rs"),
            vec![
                (ProviderName("rust"), CoverageGap::StaleSource),
                (
                    ProviderName("other"),
                    CoverageGap::OutsideRoot { root: root() },
                ),
            ],
        );

        let gaps = error.gaps();

        assert_eq!(gaps.len(), 2);
        assert_eq!(gaps[0].0, ProviderName("rust"));
        assert_eq!(gaps[0].1, CoverageGap::StaleSource);
        assert_eq!(gaps[1].0, ProviderName("other"));
        assert_eq!(gaps[1].1, CoverageGap::OutsideRoot { root: root() });
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<UncoveredFile>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<UncoveredFile>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<UncoveredFile>();
    }
}
