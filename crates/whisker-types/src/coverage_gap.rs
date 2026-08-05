use std::path::Path;
use std::sync::Arc;

/// Why a decoration provider declined to decorate a file
///
/// A gap is not a failure: the provider reports why it cannot analyze
/// the file. Each variant implies a different remedy, which
/// [`CoverageGap::help`] returns.
///
/// The enum is not `#[non_exhaustive]`: a new variant must break every
/// match that renders a gap.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum CoverageGap {
    /// The file is outside the root the provider loaded
    OutsideRoot {
        /// The root the provider loaded
        root: Arc<Path>,
    },

    /// The file is inside the root, but nothing the toolchain loaded
    /// reaches it
    Unreachable {
        /// The root the provider loaded
        root: Arc<Path>,
    },

    /// The toolchain was configured to exclude the file
    ExcludedByToolchain {
        /// The root the provider loaded
        root: Arc<Path>,
    },

    /// The text whisker parsed differs from the text the toolchain
    /// analyzed, so byte offsets from one do not address the other
    StaleSource,
}

impl CoverageGap {
    /// Returns the remedy to suggest to the user for this gap
    ///
    /// The remedy is separate from [`Display`] so a caller that reports
    /// many gaps can de-duplicate the remedies.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::CoverageGap;
    ///
    /// assert_eq!(CoverageGap::StaleSource.help(), "re-run whisker");
    /// ```
    ///
    /// [`Display`]: std::fmt::Display
    pub fn help(&self) -> &'static str {
        match self {
            Self::OutsideRoot { .. } => {
                "check the file from inside its own project, or exclude it from whisker"
            }
            Self::Unreachable { .. } => {
                "reference the file from a source the toolchain already loads, or exclude it from whisker"
            }
            Self::ExcludedByToolchain { .. } => {
                "remove the file from the toolchain's exclusion list, or exclude it from whisker"
            }
            Self::StaleSource => "re-run whisker",
        }
    }
}

impl std::fmt::Display for CoverageGap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutsideRoot { root } => write!(
                f,
                "the file is outside {}, the root the provider loaded",
                root.display()
            ),
            Self::Unreachable { root } => write!(
                f,
                "the file is inside {}, but nothing the toolchain loaded reaches it",
                root.display()
            ),
            Self::ExcludedByToolchain { root } => write!(
                f,
                "the toolchain loaded at {} was configured to exclude this file",
                root.display()
            ),
            Self::StaleSource => {
                f.write_str("the text whisker parsed differs from the text the toolchain analyzed")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn root() -> Arc<Path> {
        Arc::from(PathBuf::from("/ws"))
    }

    #[test]
    fn display_with_excluded_by_toolchain_names_root() {
        let gap = CoverageGap::ExcludedByToolchain { root: root() };

        let message = gap.to_string();

        assert!(message.contains("/ws"), "unexpected message: {message}");
        assert!(message.contains("exclude"), "unexpected message: {message}");
    }

    #[test]
    fn display_with_outside_root_names_root() {
        let gap = CoverageGap::OutsideRoot { root: root() };

        let message = gap.to_string();

        assert!(message.contains("/ws"), "unexpected message: {message}");
        assert!(
            message.contains("the root the provider loaded"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn display_with_stale_source_names_the_mismatch() {
        let gap = CoverageGap::StaleSource;

        let message = gap.to_string();

        assert!(message.contains("differs"), "unexpected message: {message}");
    }

    #[test]
    fn display_with_unreachable_names_root() {
        let gap = CoverageGap::Unreachable { root: root() };

        let message = gap.to_string();

        assert!(message.contains("/ws"), "unexpected message: {message}");
        assert!(
            message.contains("nothing the toolchain loaded reaches it"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn display_writes_a_single_line() {
        let gaps = [
            CoverageGap::OutsideRoot { root: root() },
            CoverageGap::Unreachable { root: root() },
            CoverageGap::ExcludedByToolchain { root: root() },
            CoverageGap::StaleSource,
        ];

        for gap in &gaps {
            let message = gap.to_string();

            assert!(!message.contains('\n'), "gap spans lines: {message}");
            assert_eq!(message.trim(), message, "gap is padded: {message}");
        }
    }

    #[test]
    fn help_with_excluded_by_toolchain_suggests_removing_the_exclusion() {
        let gap = CoverageGap::ExcludedByToolchain { root: root() };

        let help = gap.help();

        assert!(help.contains("exclusion list"), "unexpected help: {help}");
    }

    #[test]
    fn help_with_outside_root_suggests_its_own_project() {
        let gap = CoverageGap::OutsideRoot { root: root() };

        let help = gap.help();

        assert!(help.contains("its own project"), "unexpected help: {help}");
    }

    #[test]
    fn help_with_stale_source_suggests_rerunning() {
        let gap = CoverageGap::StaleSource;

        let help = gap.help();

        assert_eq!(help, "re-run whisker");
    }

    #[test]
    fn help_with_unreachable_suggests_referencing_the_file() {
        let gap = CoverageGap::Unreachable { root: root() };

        let help = gap.help();

        assert!(
            help.contains("reference the file from a source"),
            "unexpected help: {help}"
        );
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CoverageGap>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CoverageGap>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CoverageGap>();
    }
}
