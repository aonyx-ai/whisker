/// A [`TypePath`] a lint can write down as a constant
///
/// Const-constructible so a rule states its subject once, at module scope,
/// beside the rule that cares about it, rather than open-coding a string
/// comparison at the point of use. Named `TypePathRef` rather than `TypeRef`
/// because `ra_ap_hir` re-exports a `TypeRef` of its own and the provider
/// imports from both.
///
/// # Examples
///
/// ```
/// use whisker_rust::decorations::{TypePath, TypePathRef};
///
/// const ANYHOW_ERROR: TypePathRef<'static> = TypePathRef::new("anyhow", &[], "Error");
///
/// assert!(TypePath::new("anyhow", [] as [&str; 0], "Error") == ANYHOW_ERROR);
/// assert!(TypePath::new("syn", ["error"], "Error") != ANYHOW_ERROR);
/// ```
///
/// [`TypePath`]: crate::decorations::TypePath
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct TypePathRef<'a> {
    krate: &'a str,
    modules: &'a [&'a str],
    name: &'a str,
}

impl<'a> TypePathRef<'a> {
    /// Creates a borrowed path usable in a `const`
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePathRef;
    ///
    /// const ANYHOW_ERROR: TypePathRef<'static> = TypePathRef::new("anyhow", &[], "Error");
    ///
    /// assert_eq!(ANYHOW_ERROR.krate(), "anyhow");
    /// ```
    pub const fn new(krate: &'a str, modules: &'a [&'a str], name: &'a str) -> Self {
        Self {
            krate,
            modules,
            name,
        }
    }

    /// Returns the name of the crate that defines the type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePathRef;
    ///
    /// let path = TypePathRef::new("syn", &["error"], "Error");
    ///
    /// assert_eq!(path.krate(), "syn");
    /// ```
    pub const fn krate(&self) -> &'a str {
        self.krate
    }

    /// Returns the module segments between the crate root and the definition
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePathRef;
    ///
    /// let path = TypePathRef::new("syn", &["error"], "Error");
    ///
    /// assert_eq!(path.modules(), &["error"]);
    /// ```
    pub const fn modules(&self) -> &'a [&'a str] {
        self.modules
    }

    /// Returns the item's own name
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePathRef;
    ///
    /// let path = TypePathRef::new("syn", &["error"], "Error");
    ///
    /// assert_eq!(path.name(), "Error");
    /// ```
    pub const fn name(&self) -> &'a str {
        self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors_return_the_constructed_parts() {
        let path = TypePathRef::new("core", &["io", "error"], "Error");

        let (krate, modules, name) = (path.krate(), path.modules(), path.name());

        assert_eq!(krate, "core");
        assert_eq!(modules, &["io", "error"]);
        assert_eq!(name, "Error");
    }

    #[test]
    fn trait_send_type_path_ref() {
        fn assert_send<T: Send>() {}
        assert_send::<TypePathRef<'static>>();
    }

    #[test]
    fn trait_sync_type_path_ref() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TypePathRef<'static>>();
    }

    #[test]
    fn trait_unpin_type_path_ref() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TypePathRef<'static>>();
    }
}
