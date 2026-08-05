/// Resolved type information attached to a tree-sitter node
///
/// Decoration providers populate this on match scrutinees, `else`
/// clauses, and `?` operands. Rules access it via
/// `node.decoration::<ResolvedType>()`.
///
/// # Examples
///
/// ```
/// use whisker_rust::decorations::ResolvedType;
///
/// let ty = ResolvedType::new("MyEnum".to_string()).with_enum(true);
///
/// assert_eq!(ty.display(), "MyEnum");
/// assert!(ty.is_enum());
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
