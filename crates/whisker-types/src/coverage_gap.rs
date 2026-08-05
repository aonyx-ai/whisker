use std::path::Path;
use std::sync::Arc;

/// Why a decoration provider declined to decorate a file
///
/// A gap is not a provider failure; it is a provider correctly reporting
/// that it has nothing truthful to say about a file. The variants are
/// distinct because each implies a different fix for the user, and the
/// message they produce is the only thing standing between the user and a
/// linter that quietly analyzes nothing.
///
/// The enum is deliberately not `#[non_exhaustive]`: a new failure mode
/// must break every match that renders a gap, rather than falling into a
/// wildcard arm that says nothing actionable.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum CoverageGap {
    /// The file lives outside the workspace root directory the provider
    /// loaded, so nothing the provider knows can apply to it
    ///
    /// The remedy is to point the provider at a workspace that contains the
    /// file, which is why this is distinct from [`CoverageGap::Unreachable`]
    /// — a file the user cannot move into the loaded workspace needs a
    /// different run, not a different module tree.
    OutsideWorkspace {
        /// The workspace root the provider loaded
        root: Arc<Path>,
    },

    /// The file lives under the workspace root but no crate in the
    /// toolchain's graph reaches it, so every semantic query about it
    /// silently resolves to nothing
    ///
    /// This covers both a file the toolchain interned without attaching to
    /// a module and a file under the root the toolchain never interned at
    /// all, such as one a build script generated after the load.
    Unreachable {
        /// The workspace root the provider loaded
        root: Arc<Path>,
    },

    /// The toolchain holds the file's identity but deliberately not its
    /// contents, because it was configured to exclude it
    ExcludedByToolchain {
        /// The workspace root the provider loaded
        root: Arc<Path>,
    },

    /// The text the pipeline parsed differs from the text the toolchain
    /// analyzed, so byte offsets from one do not address the other
    StaleSource,
}

impl CoverageGap {
    /// Returns the remedy to suggest to the user for this gap
    ///
    /// The remedy is separate from [`Display`] because a caller that
    /// reports many gaps at once wants every reason but only the distinct
    /// remedies, which it can collect by de-duplicating these strings.
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
            Self::OutsideWorkspace { .. } => {
                "check the file from inside its own workspace, or exclude it from whisker"
            }
            Self::Unreachable { .. } => {
                "add the file to a crate's module tree, or exclude it from whisker"
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
            Self::OutsideWorkspace { root } => write!(
                f,
                "the file is outside the workspace loaded at {}",
                root.display()
            ),
            Self::Unreachable { root } => write!(
                f,
                "the file is inside the workspace at {} but no crate in that workspace reaches it",
                root.display()
            ),
            Self::ExcludedByToolchain { root } => write!(
                f,
                "the toolchain loaded at {} was configured to exclude this file",
                root.display()
            ),
            Self::StaleSource => f.write_str("the file changed after the workspace was loaded"),
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
    fn display_with_outside_workspace_names_root() {
        let gap = CoverageGap::OutsideWorkspace { root: root() };

        let message = gap.to_string();

        assert!(message.contains("/ws"), "unexpected message: {message}");
        assert!(
            message.contains("outside the workspace"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn display_with_stale_source_names_change() {
        let gap = CoverageGap::StaleSource;

        let message = gap.to_string();

        assert!(message.contains("changed"), "unexpected message: {message}");
    }

    #[test]
    fn display_with_unreachable_names_root() {
        let gap = CoverageGap::Unreachable { root: root() };

        let message = gap.to_string();

        assert!(message.contains("/ws"), "unexpected message: {message}");
        assert!(
            message.contains("no crate in that workspace reaches it"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn display_writes_a_single_line() {
        let gaps = [
            CoverageGap::OutsideWorkspace { root: root() },
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
    fn help_with_outside_workspace_suggests_its_own_workspace() {
        let gap = CoverageGap::OutsideWorkspace { root: root() };

        let help = gap.help();

        assert!(
            help.contains("its own workspace"),
            "unexpected help: {help}"
        );
    }

    #[test]
    fn help_with_stale_source_suggests_rerunning() {
        let gap = CoverageGap::StaleSource;

        let help = gap.help();

        assert_eq!(help, "re-run whisker");
    }

    #[test]
    fn help_with_unreachable_suggests_a_module_tree() {
        let gap = CoverageGap::Unreachable { root: root() };

        let help = gap.help();

        assert!(help.contains("module tree"), "unexpected help: {help}");
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
