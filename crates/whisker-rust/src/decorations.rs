/// Resolved type information attached to expression nodes
///
/// Decoration providers populate this on nodes where lints need type
/// information — match scrutinees, function arguments, `?` operands,
/// and similar. Rules access it via
/// `node.decoration::<ResolvedType>()`.
///
/// # Examples
///
/// ```ignore
/// if let Some(ty) = node.decoration::<ResolvedType>() {
///     if ty.is_enum {
///         // scrutinee is an enum
///     }
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ResolvedType {
    display: String,
    is_enum: bool,
    is_never: bool,
    is_result: bool,
    is_option: bool,
}

impl ResolvedType {
    /// Creates a new resolved type
    pub fn new(display: String) -> Self {
        Self {
            display,
            is_enum: false,
            is_never: false,
            is_result: false,
            is_option: false,
        }
    }

    /// Marks this type as an enum
    pub fn with_enum(mut self, is_enum: bool) -> Self {
        self.is_enum = is_enum;
        self
    }

    /// Marks this type as the never type (`!`)
    pub fn with_never(mut self, is_never: bool) -> Self {
        self.is_never = is_never;
        self
    }

    /// Marks this type as `Result<T, E>`
    pub fn with_result(mut self, is_result: bool) -> Self {
        self.is_result = is_result;
        self
    }

    /// Marks this type as `Option<T>`
    pub fn with_option(mut self, is_option: bool) -> Self {
        self.is_option = is_option;
        self
    }

    /// Returns the human-readable type name
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Returns whether this is an enum type
    pub fn is_enum(&self) -> bool {
        self.is_enum
    }

    /// Returns whether this is the never type (`!`)
    pub fn is_never(&self) -> bool {
        self.is_never
    }

    /// Returns whether this is a `Result<T, E>` type
    pub fn is_result(&self) -> bool {
        self.is_result
    }

    /// Returns whether this is an `Option<T>` type
    pub fn is_option(&self) -> bool {
        self.is_option
    }
}

/// ADT-specific flags for enum types
///
/// Attached alongside [`ResolvedType`] when the type is an enum.
/// Provides additional information about the ADT that rules need for
/// decisions like whether a wildcard match arm is acceptable.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct AdtFlags {
    non_exhaustive_external: bool,
}

impl AdtFlags {
    /// Creates ADT flags
    pub fn new(non_exhaustive_external: bool) -> Self {
        Self {
            non_exhaustive_external,
        }
    }

    /// Returns whether the enum is `#[non_exhaustive]` from an external crate
    pub fn non_exhaustive_external(&self) -> bool {
        self.non_exhaustive_external
    }
}

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
    fn trait_send_resolved_type() {
        fn assert_send<T: Send>() {}
        assert_send::<ResolvedType>();
    }

    #[test]
    fn trait_sync_resolved_type() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ResolvedType>();
    }

    #[test]
    fn trait_unpin_resolved_type() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ResolvedType>();
    }

    #[test]
    fn trait_send_adt_flags() {
        fn assert_send<T: Send>() {}
        assert_send::<AdtFlags>();
    }

    #[test]
    fn trait_sync_adt_flags() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AdtFlags>();
    }

    #[test]
    fn trait_unpin_adt_flags() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AdtFlags>();
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

    #[test]
    fn resolved_type_builder_defaults_to_false() {
        let ty = ResolvedType::new("i32".into());

        assert_eq!(ty.display(), "i32");
        assert!(!ty.is_enum());
        assert!(!ty.is_never());
        assert!(!ty.is_result());
        assert!(!ty.is_option());
    }

    #[test]
    fn resolved_type_builder_sets_flags() {
        let ty = ResolvedType::new("MyEnum".into())
            .with_enum(true)
            .with_never(false);

        assert!(ty.is_enum());
        assert!(!ty.is_never());
    }

    #[test]
    fn resolved_type_result_and_option_flags() {
        let result_ty = ResolvedType::new("Result<T, E>".into()).with_result(true);
        let option_ty = ResolvedType::new("Option<T>".into()).with_option(true);

        assert!(result_ty.is_result());
        assert!(!result_ty.is_option());
        assert!(option_ty.is_option());
        assert!(!option_ty.is_result());
    }

    #[test]
    fn adt_flags_accessors() {
        let flags = AdtFlags::new(true);
        assert!(flags.non_exhaustive_external());

        let flags = AdtFlags::new(false);
        assert!(!flags.non_exhaustive_external());
    }

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
}
