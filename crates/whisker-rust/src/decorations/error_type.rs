use crate::decorations::{TypePath, TypePathRef};

/// The `E` of the [`Result<T, E>`] a function returns
///
/// Not every `E` has a defining item. A type parameter, a reference, or a
/// tuple has none, so no [`TypePath`] exists for it.
///
/// `Box<dyn Error>` is [`ErrorType::Named`] with the path
/// `alloc::boxed::Box`, because `Box` is the ADT in the `E` slot.
///
/// [`Result<T, E>`]: std::result::Result
/// [`TypePath`]: crate::decorations::TypePath
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum ErrorType {
    /// The error is an ADT, identified by where it is defined
    Named(TypePath),
    /// The error is a type parameter, not a concrete type
    Generic,
    /// The error is a type with no defining item
    Unnamed,
}

impl ErrorType {
    /// Returns whether the error type is the one `path` names
    ///
    /// The comparison uses definition paths, not rendered names, so
    /// `syn::Error` does not match `anyhow::Error`.
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::{ErrorType, TypePath, TypePathRef};
    ///
    /// const ANYHOW_ERROR: TypePathRef<'static> = TypePathRef::new("anyhow", &[], "Error");
    ///
    /// let error = ErrorType::Named(TypePath::new("syn", ["error"], "Error"));
    ///
    /// assert!(!error.is(ANYHOW_ERROR));
    /// ```
    pub fn is(&self, path: TypePathRef<'_>) -> bool {
        match self {
            ErrorType::Named(actual) => *actual == path,
            ErrorType::Generic => false,
            ErrorType::Unnamed => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANYHOW_ERROR: TypePathRef<'static> = TypePathRef::new("anyhow", &[], "Error");

    #[test]
    fn is_with_generic_returns_false() {
        let error = ErrorType::Generic;

        let matches = error.is(ANYHOW_ERROR);

        assert!(!matches);
    }

    #[test]
    fn is_with_matching_path_returns_true() {
        let error = ErrorType::Named(TypePath::new("anyhow", [] as [&str; 0], "Error"));

        let matches = error.is(ANYHOW_ERROR);

        assert!(matches);
    }

    /// Pins that `syn::Error` does not match a path that names `anyhow::Error`
    ///
    /// Both render as `Error`, so only the definition path distinguishes them.
    #[test]
    fn is_with_same_name_in_another_crate_returns_false() {
        let error = ErrorType::Named(TypePath::new("syn", ["error"], "Error"));

        let matches = error.is(ANYHOW_ERROR);

        assert!(!matches);
    }

    #[test]
    fn is_with_unnamed_returns_false() {
        let error = ErrorType::Unnamed;

        let matches = error.is(ANYHOW_ERROR);

        assert!(!matches);
    }

    #[test]
    fn trait_send_error_type() {
        fn assert_send<T: Send>() {}
        assert_send::<ErrorType>();
    }

    #[test]
    fn trait_sync_error_type() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ErrorType>();
    }

    #[test]
    fn trait_unpin_error_type() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ErrorType>();
    }
}
