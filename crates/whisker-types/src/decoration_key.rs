/// Identifies a decoration type across separately compiled crate graphs
///
/// The decoration map stores type-erased values and must decide, at
/// retrieval, whether a stored value is of the requested type. [`TypeId`]
/// cannot make that call: it incorporates the compiler's per-crate metadata,
/// so the same source compiled into the whisker binary and into a custom
/// lint plugin produces two different ids, and a plugin's lookup would
/// silently miss every decoration the host recorded. A key compared by
/// string content is identical in both images whenever both were compiled
/// from the same source, which the plugin handshake enforces.
///
/// Key equality is the decoration map's license to cast an erased value
/// back to a concrete type, so a key must name exactly one type definition.
/// That contract belongs to [`Decoration`], which is an unsafe trait for
/// this reason. The derive macro builds a key that holds to it from the
/// type's module path, its name, and a hash of its definition, so two
/// types stay apart even where a module path and a name coincide.
///
/// # Examples
///
/// ```
/// use whisker_types::DecorationKey;
///
/// const KEY: DecorationKey = DecorationKey::new(concat!(module_path!(), "::Signature"));
///
/// assert!(KEY.as_str().ends_with("::Signature"));
/// ```
///
/// [`Decoration`]: crate::Decoration
/// [`TypeId`]: std::any::TypeId
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct DecorationKey(&'static str);

impl DecorationKey {
    /// Creates a key from the name that identifies a decoration type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::DecorationKey;
    ///
    /// const KEY: DecorationKey = DecorationKey::new("my_crate::Signature");
    /// ```
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Returns the name this key was created from
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_types::DecorationKey;
    ///
    /// let key = DecorationKey::new("my_crate::Signature");
    ///
    /// assert_eq!(key.as_str(), "my_crate::Signature");
    /// ```
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_compares_content_not_address() {
        let literal = DecorationKey::new("tests::Marker");
        let concatenated = DecorationKey::new(concat!("tests", "::Marker"));

        assert_eq!(literal, concatenated);
    }

    #[test]
    fn keys_with_different_names_are_unequal() {
        let first = DecorationKey::new("tests::First");
        let second = DecorationKey::new("tests::Second");

        assert_ne!(first, second);
    }

    #[test]
    fn new_roundtrips_through_as_str() {
        let key = DecorationKey::new("tests::Marker");

        assert_eq!(key.as_str(), "tests::Marker");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DecorationKey>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DecorationKey>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DecorationKey>();
    }
}
