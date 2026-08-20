use std::mem::offset_of;

use crate::Span;

/// A suggested source code replacement
///
/// Represents a machine-applicable fix: replace the text at `span` with
/// `replacement`. An empty replacement means "delete this span".
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Suggestion {
    span: Span,
    replacement: String,
    message: String,
}

impl Suggestion {
    /// Creates a suggestion to replace the text at `span` with `replacement`
    pub fn new(span: Span, replacement: String, message: String) -> Self {
        Self {
            span,
            replacement,
            message,
        }
    }

    /// Returns the span of the text to be replaced
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns the replacement text
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// Returns the human-readable description of this suggestion
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// The offsets of every field, in declaration order
///
/// The plugin handshake hashes these so a plugin that places a field
/// somewhere else is refused rather than trusted. They live beside the
/// struct, because a field added there has to be added here too.
pub(crate) const FIELD_OFFSETS: &[usize] = &[
    offset_of!(Suggestion, span),
    offset_of!(Suggestion, replacement),
    offset_of!(Suggestion, message),
];

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Suggestion>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Suggestion>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Suggestion>();
    }

    #[test]
    fn accessors_return_correct_values() {
        let span = Span::new(PathBuf::from("test.rs"), 0, 5);
        let suggestion = Suggestion::new(span.clone(), "new_text".into(), "try this".into());

        assert_eq!(suggestion.span(), &span);
        assert_eq!(suggestion.replacement(), "new_text");
        assert_eq!(suggestion.message(), "try this");
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn new_roundtrips_all_fields(
                start in 0..=1000usize,
                delta in 0..=1000usize,
                replacement in "\\PC{0,50}",
                message in "\\PC{0,50}",
            ) {
                let span = Span::new(PathBuf::from("f.rs"), start, start + delta);
                let s = Suggestion::new(span.clone(), replacement.clone(), message.clone());

                prop_assert_eq!(s.span(), &span);
                prop_assert_eq!(s.replacement(), replacement);
                prop_assert_eq!(s.message(), message);
            }
        }
    }
}
