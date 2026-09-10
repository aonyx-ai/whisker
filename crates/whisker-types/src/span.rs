use std::path::Path;

use crate::FilePath;

/// A byte range within a source file
///
/// Spans identify a contiguous region of source text by file path and byte
/// offsets. The range is half-open: `[start, end)`. The file path is
/// reference-counted so that creating spans from a shared source is cheap.
///
/// A span crosses the plugin boundary inside every [`Diagnostic`], so
/// stabby lays it out. Its file is a [`FilePath`], for the reason that
/// type gives.
///
/// [`Diagnostic`]: crate::Diagnostic
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Span {
    file: FilePath,
    start: usize,
    end: usize,
}

impl Span {
    /// Creates a span covering `[start, end)` in the given file
    ///
    /// # Panics
    ///
    /// Panics if `start > end`.
    pub fn new(file: impl Into<FilePath>, start: usize, end: usize) -> Self {
        let file = file.into();
        assert!(
            start <= end,
            "span start ({start}) must not exceed end ({end})"
        );
        Self { file, start, end }
    }

    /// Returns the file path this span belongs to
    pub fn file(&self) -> &Path {
        self.file.as_path()
    }

    /// Returns the shared file path
    ///
    /// Clone it to build another span in the same file without copying
    /// the path.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::Span;
    ///
    /// let span = Span::new("src/lib.rs", 10, 20);
    ///
    /// let narrower = Span::new(span.file_path().clone(), 12, 14);
    ///
    /// assert_eq!(narrower.file(), span.file());
    /// ```
    pub fn file_path(&self) -> &FilePath {
        &self.file
    }

    /// Returns the start byte offset (inclusive)
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the end byte offset (exclusive)
    pub fn end(&self) -> usize {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Span>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Span>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Span>();
    }

    #[test]
    fn accessors_return_correct_values() {
        let span = Span::new(PathBuf::from("test.rs"), 10, 20);

        assert_eq!(span.file(), Path::new("test.rs"));
        assert_eq!(span.start(), 10);
        assert_eq!(span.end(), 20);
    }

    #[test]
    fn empty_span_is_allowed() {
        let span = Span::new(PathBuf::from("test.rs"), 5, 5);
        assert_eq!(span.start(), span.end());
    }

    #[test]
    fn file_path_is_shared_by_a_clone() {
        let span = Span::new(PathBuf::from("test.rs"), 0, 10);

        let copy = span.clone();

        assert!(std::ptr::eq(copy.file(), span.file()));
    }

    #[test]
    fn new_accepts_a_shared_file_path() {
        let span = Span::new(PathBuf::from("test.rs"), 0, 10);

        let narrower = Span::new(span.file_path().clone(), 2, 4);

        assert_eq!(narrower.file(), Path::new("test.rs"));
        assert_eq!(narrower.start(), 2);
        assert_eq!(narrower.end(), 4);
    }

    #[test]
    #[should_panic(expected = "span start")]
    fn new_with_inverted_range_panics() {
        Span::new(PathBuf::from("test.rs"), 20, 10);
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn new_roundtrips_all_fields(
                file in "[a-z]{1,10}\\.rs",
                start in 0..=1000usize,
                delta in 0..=1000usize,
            ) {
                let end = start + delta;
                let span = Span::new(PathBuf::from(&file), start, end);

                prop_assert_eq!(span.file(), Path::new(&file));
                prop_assert_eq!(span.start(), start);
                prop_assert_eq!(span.end(), end);
            }

            #[test]
            fn start_never_exceeds_end(
                file in "[a-z]+\\.rs",
                start in 0..=1000usize,
                delta in 0..=1000usize,
            ) {
                let end = start + delta;
                let span = Span::new(PathBuf::from(&file), start, end);
                prop_assert!(span.start() <= span.end());
            }
        }
    }
}
