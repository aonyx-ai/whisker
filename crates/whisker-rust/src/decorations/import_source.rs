use whisker_macros::Decoration;

/// What the qualifier of a `use` path names
///
/// Spelling does not answer this. `Message` in `use Message::Ping` names an
/// enum, and an uppercase name such as `Shapes` can still name a module.
///
/// The provider attaches it to a `use_declaration` whose path has a
/// qualifier. `use serde;` has none, so it carries no decoration.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Decoration)]
#[decoration(cardinality = "one")]
pub enum ImportSource {
    /// A module or a crate root
    Module,
    /// An enum, so the import names its variants
    Enum,
    /// Anything else, such as a struct or a trait
    Other,
    /// A qualifier rust-analyzer resolved to nothing
    Unresolved,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_send_import_source() {
        fn assert_send<T: Send>() {}
        assert_send::<ImportSource>();
    }

    #[test]
    fn trait_sync_import_source() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ImportSource>();
    }

    #[test]
    fn trait_unpin_import_source() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ImportSource>();
    }
}
