use crate::decorations::{TypePath, TypePathRef};

/// The `E` of the [`Result<T, E>`] a function returns
///
/// Not every `E` has a name. A type parameter, a reference, and a tuple are
/// all types rust-analyzer resolved perfectly well, and none of them is
/// defined by an item that could be pointed at, so there is nothing to
/// compare a [`TypePath`] against. Saying so is better than inventing a name
/// for them.
///
/// `Box<dyn Error>` is [`ErrorType::Named`], not something dynamic: the `E`
/// slot holds `Box`, which is an ADT, so its identity is `alloc::boxed::Box`.
/// That is factually right and produces the right lint outcome, but it will
/// surprise a reader expecting a trait-object variant. An unsized
/// `dyn Error` cannot occupy the `E` slot at all, which is why there is no
/// such variant.
///
/// [`Result<T, E>`]: std::result::Result
/// [`TypePath`]: crate::decorations::TypePath
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum ErrorType {
    /// The error is an ADT, identified by where it is defined
    Named(TypePath),
    /// The error is a type parameter the signature does not pin down
    Generic,
    /// The error resolved to something with no defining item to name
    Unnamed,
}

impl ErrorType {
    /// Returns whether the error type is the one `path` names
    ///
    /// The comparison is between definitions rust-analyzer resolved, not
    /// names it printed, so a crate's own `Error` never answers to
    /// `anyhow::Error`.
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

    /// A crate's own `Error` must not answer to anyhow's
    ///
    /// This is the whole point of the type: `syn::Error`, `std::io::Error`,
    /// and `anyhow::Error` all render as the bare word `Error`, so a rule
    /// comparing renderings flags all three.
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
