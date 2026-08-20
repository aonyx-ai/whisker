/// One line of doc comment text, with the marker gone
///
/// A sentence often runs across several lines, so the reassembled prose has
/// offsets of its own. The file offset of each line lets a diagnostic point
/// at the sentence rather than at the whole comment.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub(crate) struct DocLine<'a> {
    text: &'a str,
    offset: usize,
}

impl<'a> DocLine<'a> {
    /// Creates a line holding `text`, which starts at `offset` in the file
    pub(crate) fn new(text: &'a str, offset: usize) -> Self {
        Self { text, offset }
    }

    /// Returns the text of the line
    pub(crate) fn text(&self) -> &'a str {
        self.text
    }

    /// Returns the byte offset of the line in the file
    pub(crate) fn offset(&self) -> usize {
        self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors_return_constructor_arguments() {
        let line = DocLine::new(" Returns the length", 7);

        assert_eq!(line.text(), " Returns the length");
        assert_eq!(line.offset(), 7);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DocLine<'_>>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DocLine<'_>>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DocLine<'_>>();
    }
}
