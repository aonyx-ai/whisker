use crate::decorations::{ErrorType, ResolvedType, ReturnMode};

/// Function signature information
///
/// Attached to `function_item` nodes so lints can inspect the return type
/// without re-querying the semantic model. Every resolved function gets a
/// signature, even an empty one. An absent decoration would look like a
/// function the provider never reached, and those are different facts.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FnSignature {
    return_type: Option<ResolvedType>,
    error_type: Option<ErrorType>,
    return_mode: ReturnMode,
}

impl FnSignature {
    /// Creates a function signature
    pub fn new(
        return_type: Option<ResolvedType>,
        error_type: Option<ErrorType>,
        return_mode: ReturnMode,
    ) -> Self {
        Self {
            return_type,
            error_type,
            return_mode,
        }
    }

    /// Returns the resolved return type, awaited for an `async fn`
    pub fn return_type(&self) -> Option<&ResolvedType> {
        self.return_type.as_ref()
    }

    /// Returns the `E` of the `Result<T, E>` this function returns
    ///
    /// [`None`] means the function does not return a `core::result::Result`.
    /// That includes an `async fn` whose awaited type is not one, and one
    /// whose future could not be projected.
    pub fn error_type(&self) -> Option<&ErrorType> {
        self.error_type.as_ref()
    }

    /// Returns where the return type was read from
    pub fn return_mode(&self) -> ReturnMode {
        self.return_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decorations::{TypePath, TypePathRef};

    #[test]
    fn fn_signature_with_return_type() {
        let ret = ResolvedType::new("Result<(), anyhow::Error>".into()).with_result(true);
        let error = ErrorType::Named(TypePath::new("anyhow", [] as [&str; 0], "Error"));

        let sig = FnSignature::new(Some(ret), Some(error), ReturnMode::Direct);

        assert!(sig.return_type().unwrap().is_result());
        assert!(
            sig.error_type()
                .unwrap()
                .is(TypePathRef::new("anyhow", &[], "Error"))
        );
        assert_eq!(sig.return_mode(), ReturnMode::Direct);
    }

    #[test]
    fn fn_signature_without_return_type() {
        let sig = FnSignature::new(None, None, ReturnMode::Opaque);

        assert!(sig.return_type().is_none());
        assert!(sig.error_type().is_none());
        assert_eq!(sig.return_mode(), ReturnMode::Opaque);
    }

    #[test]
    fn trait_send_fn_signature() {
        fn assert_send<T: Send>() {}
        assert_send::<FnSignature>();
    }

    #[test]
    fn trait_sync_fn_signature() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FnSignature>();
    }

    #[test]
    fn trait_unpin_fn_signature() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FnSignature>();
    }
}
