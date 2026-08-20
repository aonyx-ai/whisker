use std::mem::offset_of;
use std::path::Path;
use std::sync::Arc;

/// A byte range within a source file
///
/// Spans identify a contiguous region of source text by file path and byte
/// offsets. The range is half-open: `[start, end)`. The file path is
/// reference-counted so that creating spans from a shared source is cheap.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Span {
    file: Arc<Path>,
    start: usize,
    end: usize,
}

impl Span {
    /// Creates a span covering `[start, end)` in the given file
    ///
    /// # Panics
    ///
    /// Panics if `start > end`.
    pub fn new(file: impl Into<Arc<Path>>, start: usize, end: usize) -> Self {
        let file = file.into();
        assert!(
            start <= end,
            "span start ({start}) must not exceed end ({end})"
        );
        Self { file, start, end }
    }

    /// Returns the file path this span belongs to
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns the shared file path
    pub fn file_arc(&self) -> &Arc<Path> {
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

/// The offsets of every field, in declaration order
///
/// The plugin handshake hashes these so a plugin that places a field
/// somewhere else is refused rather than trusted. They live beside the
/// struct, because a field added there has to be added here too.
pub(crate) const FIELD_OFFSETS: &[usize] = &[
    offset_of!(Span, file),
    offset_of!(Span, start),
    offset_of!(Span, end),
];

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
    #[should_panic(expected = "span start")]
    fn new_with_inverted_range_panics() {
        Span::new(PathBuf::from("test.rs"), 20, 10);
    }

    #[test]
    fn file_arc_shares_reference() {
        let span1 = Span::new(PathBuf::from("test.rs"), 0, 10);
        let span2 = span1.clone();
        assert!(Arc::ptr_eq(span1.file_arc(), span2.file_arc()));
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
