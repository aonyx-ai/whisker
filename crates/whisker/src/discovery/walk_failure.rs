use ignore::Error;

/// What a step of the walk was unable to read
///
/// The `ignore` crate reports an unparsable ignore file in two ways. A file
/// inside the walk root arrives attached to its directory entry; a file
/// above the root arrives as a plain walk failure. This type classifies a
/// plain walk failure, where the error alone must say what failed.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum WalkFailure {
    /// An entry of the tree could not be read
    DirectoryEntry,

    /// An ignore file could not be understood
    IgnoreFile,
}

impl WalkFailure {
    /// Returns the kind of failure `error` describes
    ///
    /// Glob errors and line-numbered errors only come from parsing an ignore
    /// file. Everything else, I/O included, counts as a directory-entry
    /// failure: the walker cannot tell an unreadable directory from an
    /// unreadable ignore file inside it.
    pub fn classify(error: &Error) -> Self {
        match error {
            Error::Glob { glob: _, err: _ } | Error::WithLineNumber { line: _, err: _ } => {
                Self::IgnoreFile
            }
            Error::WithPath { path: _, err } | Error::WithDepth { depth: _, err } => {
                Self::classify(err)
            }
            Error::Partial(errors) => {
                let has_ignore_file = errors
                    .iter()
                    .any(|error| Self::classify(error) == Self::IgnoreFile);

                match has_ignore_file {
                    true => Self::IgnoreFile,
                    false => Self::DirectoryEntry,
                }
            }
            Error::Loop {
                ancestor: _,
                child: _,
            }
            | Error::Io(_)
            | Error::UnrecognizedFileType(_)
            | Error::InvalidDefinition => Self::DirectoryEntry,
        }
    }

    /// Returns the context the caller reports this failure under
    ///
    /// The caller attaches this string to the walker's own error. That error
    /// stays the cause, so the path and reason it carries reach the user.
    pub fn context(self) -> &'static str {
        match self {
            Self::DirectoryEntry => "failed to read a directory entry",
            Self::IgnoreFile => "failed to read an ignore file",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use super::*;

    /// Returns a glob failure of the shape an unparsable ignore file produces
    fn glob_error() -> Error {
        Error::Glob {
            glob: Some("{a,b".to_owned()),
            err: "unclosed alternate group".to_owned(),
        }
    }

    #[test]
    fn classify_with_a_glob_error_returns_ignore_file() {
        let error = glob_error();

        let failure = WalkFailure::classify(&error);

        assert_eq!(failure, WalkFailure::IgnoreFile);
    }

    #[test]
    fn classify_with_a_line_numbered_error_returns_ignore_file() {
        let error = Error::WithLineNumber {
            line: 1,
            err: Box::new(glob_error()),
        };

        let failure = WalkFailure::classify(&error);

        assert_eq!(failure, WalkFailure::IgnoreFile);
    }

    #[test]
    fn classify_with_a_loop_error_returns_directory_entry() {
        let error = Error::Loop {
            ancestor: PathBuf::from("/project"),
            child: PathBuf::from("/project/link"),
        };

        let failure = WalkFailure::classify(&error);

        assert_eq!(failure, WalkFailure::DirectoryEntry);
    }

    #[test]
    fn classify_with_a_nested_glob_error_returns_ignore_file() {
        let error = Error::WithPath {
            path: PathBuf::from("/project/.gitignore"),
            err: Box::new(Error::WithLineNumber {
                line: 1,
                err: Box::new(glob_error()),
            }),
        };

        let failure = WalkFailure::classify(&error);

        assert_eq!(failure, WalkFailure::IgnoreFile);
    }

    #[test]
    fn classify_with_a_nested_io_error_returns_directory_entry() {
        let error = Error::WithDepth {
            depth: 2,
            err: Box::new(Error::WithPath {
                path: PathBuf::from("/project/locked"),
                err: Box::new(Error::Io(io::Error::from(io::ErrorKind::PermissionDenied))),
            }),
        };

        let failure = WalkFailure::classify(&error);

        assert_eq!(failure, WalkFailure::DirectoryEntry);
    }

    #[test]
    fn classify_with_a_partial_error_holding_a_glob_error_returns_ignore_file() {
        let error = Error::Partial(vec![
            Error::Io(io::Error::from(io::ErrorKind::NotFound)),
            glob_error(),
        ]);

        let failure = WalkFailure::classify(&error);

        assert_eq!(failure, WalkFailure::IgnoreFile);
    }

    #[test]
    fn classify_with_a_partial_error_holding_no_glob_error_returns_directory_entry() {
        let error = Error::Partial(vec![Error::Io(io::Error::from(io::ErrorKind::NotFound))]);

        let failure = WalkFailure::classify(&error);

        assert_eq!(failure, WalkFailure::DirectoryEntry);
    }

    #[test]
    fn context_with_directory_entry_names_the_entry() {
        let context = WalkFailure::DirectoryEntry.context();

        assert_eq!(context, "failed to read a directory entry");
    }

    #[test]
    fn context_with_ignore_file_names_the_ignore_file() {
        let context = WalkFailure::IgnoreFile.context();

        assert_eq!(context, "failed to read an ignore file");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WalkFailure>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<WalkFailure>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<WalkFailure>();
    }
}
