use std::mem::offset_of;

use crate::Span;

/// A source location with an associated message
///
/// Used for origin and related annotations on diagnostics. The message
/// describes the role this location plays in the diagnostic (e.g. "defined
/// here" or "first occurrence").
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Location {
    span: Span,
    message: String,
}

impl Location {
    /// Creates a location with the given span and descriptive message
    pub fn new(span: Span, message: String) -> Self {
        Self { span, message }
    }

    /// Returns the span of this location
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Returns the descriptive message for this location
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// The offsets of every field, in declaration order
///
/// The plugin handshake hashes these so a plugin that places a field
/// somewhere else is refused rather than trusted. They live beside the
/// struct, because a field added there has to be added here too.
pub(crate) const FIELD_OFFSETS: &[usize] =
    &[offset_of!(Location, span), offset_of!(Location, message)];

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Location>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Location>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Location>();
    }

    #[test]
    fn accessors_return_correct_values() {
        let span = Span::new(PathBuf::from("test.rs"), 0, 10);
        let location = Location::new(span.clone(), "defined here".into());

        assert_eq!(location.span(), &span);
        assert_eq!(location.message(), "defined here");
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn new_roundtrips_all_fields(
                file in "[a-z]+\\.rs",
                start in 0..=1000usize,
                delta in 0..=1000usize,
                message in "\\PC{0,50}",
            ) {
                let span = Span::new(PathBuf::from(&file), start, start + delta);
                let location = Location::new(span.clone(), message.clone());

                prop_assert_eq!(location.span(), &span);
                prop_assert_eq!(location.message(), message);
            }
        }
    }
}
