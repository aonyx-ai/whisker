use crate::decorations::ResolvedType;

/// Function signature information
///
/// Attached to function item nodes so lints can inspect the return type
/// without re-querying the semantic model.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FnSignature {
    return_type: Option<ResolvedType>,
    error_type_name: Option<String>,
}

impl FnSignature {
    /// Creates a function signature
    pub fn new(return_type: Option<ResolvedType>, error_type_name: Option<String>) -> Self {
        Self {
            return_type,
            error_type_name,
        }
    }

    /// Returns the resolved return type, if available
    pub fn return_type(&self) -> Option<&ResolvedType> {
        self.return_type.as_ref()
    }

    /// Returns the error type name for `Result`-returning functions
    ///
    /// This is the fully qualified name of the `E` in `Result<T, E>`,
    /// e.g. `"anyhow::Error"`.
    pub fn error_type_name(&self) -> Option<&str> {
        self.error_type_name.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fn_signature_with_return_type() {
        let ret = ResolvedType::new("Result<(), anyhow::Error>".into()).with_result(true);
        let sig = FnSignature::new(Some(ret), Some("anyhow::Error".into()));

        assert!(sig.return_type().unwrap().is_result());
        assert_eq!(sig.error_type_name(), Some("anyhow::Error"));
    }

    #[test]
    fn fn_signature_without_return_type() {
        let sig = FnSignature::new(None, None);

        assert!(sig.return_type().is_none());
        assert!(sig.error_type_name().is_none());
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
