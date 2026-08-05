/// ADT-specific flags for enum types
///
/// Attached alongside [`ResolvedType`] when the type is an enum.
/// Provides additional information about the ADT that rules need for
/// decisions like whether a wildcard match arm is acceptable.
///
/// [`ResolvedType`]: crate::decorations::ResolvedType
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adt_flags_accessors() {
        let flags = AdtFlags::new(true);
        assert!(flags.non_exhaustive_external());

        let flags = AdtFlags::new(false);
        assert!(!flags.non_exhaustive_external());
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
}
