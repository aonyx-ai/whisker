use std::fmt;

use crate::decorations::TypePathRef;

/// Where a type is defined, as a path the compiler would recognize
///
/// A display string cannot answer "is this `anyhow::Error`". Rust-analyzer
/// renders `syn::Error`, `std::io::Error`, `std::fmt::Error`, and
/// `anyhow::Error` all as `Error`, because it renders types the way a
/// programmer would write them at a use site, where the surrounding imports
/// supply what the text leaves out. A lint has no use site and no imports,
/// so it needs the identity the renderer discarded.
///
/// Two tempting alternatives do not work, and it is worth saying why.
/// Asking whether the type implements [`Error`] discriminates nothing:
/// `syn::Error`, `serde_json::Error`, `std::io::Error`, and everything
/// `anyhow::Error` wraps all implement it. Asking whether `.context()` can
/// be called on it also fails, because `anyhow::Context` is implemented for
/// [`Result<T, E>`] for every `E: Error + Send + Sync + 'static`, so the
/// method resolves perfectly well inside a `syn::Result` function. What
/// actually stops you writing `.context(..)?` there is the return
/// conversion, not method applicability. Nominal identity is the honest
/// primitive.
///
/// The path names the item's *definition*, not any public re-export of it.
/// `syn::Error` reads back as `syn::error::Error`, because that is where
/// `syn` declares it; the root re-export leaves no trace here. Comparing
/// definition paths is what makes the comparison stable — a crate may add,
/// move, or drop a re-export in a patch release, but moving a definition is
/// a breaking change. A rule that genuinely needs the public path instead
/// would have to resolve it through rust-analyzer's module scope, which
/// follows `pub use`; nothing needs that today.
///
/// # Examples
///
/// ```
/// use whisker_rust::decorations::TypePath;
///
/// let path = TypePath::new("syn", ["error"], "Error");
///
/// assert_eq!(path.to_string(), "syn::error::Error");
/// ```
///
/// [`Error`]: std::error::Error
/// [`Result<T, E>`]: std::result::Result
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct TypePath {
    krate: Box<str>,
    modules: Box<[Box<str>]>,
    name: Box<str>,
}

impl TypePath {
    /// Creates a path from a crate name, its module segments, and an item name
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePath;
    ///
    /// let path = TypePath::new("anyhow", [] as [&str; 0], "Error");
    ///
    /// assert_eq!(path.to_string(), "anyhow::Error");
    /// ```
    pub fn new<M>(krate: &str, modules: M, name: &str) -> Self
    where
        M: IntoIterator,
        M::Item: AsRef<str>,
    {
        Self {
            krate: Box::from(krate),
            modules: modules
                .into_iter()
                .map(|segment| Box::from(segment.as_ref()))
                .collect(),
            name: Box::from(name),
        }
    }

    /// Returns the name of the crate that defines the type
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePath;
    ///
    /// let path = TypePath::new("syn", ["error"], "Error");
    ///
    /// assert_eq!(path.krate(), "syn");
    /// ```
    pub fn krate(&self) -> &str {
        &self.krate
    }

    /// Returns the module segments between the crate root and the definition
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePath;
    ///
    /// let path = TypePath::new("syn", ["error"], "Error");
    ///
    /// assert_eq!(path.modules().collect::<Vec<_>>(), vec!["error"]);
    /// ```
    pub fn modules(&self) -> impl Iterator<Item = &str> {
        self.modules.iter().map(AsRef::as_ref)
    }

    /// Returns the item's own name
    ///
    /// # Examples
    ///
    /// ```
    /// use whisker_rust::decorations::TypePath;
    ///
    /// let path = TypePath::new("syn", ["error"], "Error");
    ///
    /// assert_eq!(path.name(), "Error");
    /// ```
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Renders the path as `krate::module::Name`, for diagnostics only
///
/// Comparison goes through [`TypePathRef`], which cannot be fooled by a path
/// that happens to render the same way.
///
/// [`TypePathRef`]: crate::decorations::TypePathRef
impl fmt::Display for TypePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.krate)?;
        for segment in &self.modules {
            write!(f, "::{segment}")?;
        }
        write!(f, "::{}", self.name)
    }
}

impl PartialEq<TypePathRef<'_>> for TypePath {
    fn eq(&self, other: &TypePathRef<'_>) -> bool {
        &*self.krate == other.krate()
            && &*self.name == other.name()
            && self.modules.len() == other.modules().len()
            && self
                .modules
                .iter()
                .zip(other.modules())
                .all(|(lhs, rhs)| &**lhs == *rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_renders_crate_modules_and_name() {
        let path = TypePath::new("syn", ["error"], "Error");

        let rendered = path.to_string();

        assert_eq!(rendered, "syn::error::Error");
    }

    #[test]
    fn display_with_no_modules_renders_crate_and_name() {
        let path = TypePath::new("anyhow", [] as [&str; 0], "Error");

        let rendered = path.to_string();

        assert_eq!(rendered, "anyhow::Error");
    }

    #[test]
    fn eq_with_different_crate_is_false() {
        let path = TypePath::new("thiserror", [] as [&str; 0], "Error");

        let matches = path == TypePathRef::new("anyhow", &[], "Error");

        assert!(!matches);
    }

    /// A prefix of the right modules must not pass for the right modules
    ///
    /// Zipping two segment lists stops at the shorter one, so without an
    /// explicit length check `core::io::Error` would answer to a path naming
    /// `core::io::error::Error` and vice versa.
    #[test]
    fn eq_with_different_module_depth_is_false() {
        let path = TypePath::new("core", ["io", "error"], "Error");

        let matches = path == TypePathRef::new("core", &["io"], "Error");

        assert!(!matches);
    }

    #[test]
    fn eq_with_different_name_is_false() {
        let path = TypePath::new("anyhow", [] as [&str; 0], "Chain");

        let matches = path == TypePathRef::new("anyhow", &[], "Error");

        assert!(!matches);
    }

    #[test]
    fn eq_with_matching_type_path_ref_is_true() {
        let path = TypePath::new("syn", ["error"], "Error");

        let matches = path == TypePathRef::new("syn", &["error"], "Error");

        assert!(matches);
    }

    #[test]
    fn trait_send_type_path() {
        fn assert_send<T: Send>() {}
        assert_send::<TypePath>();
    }

    #[test]
    fn trait_sync_type_path() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TypePath>();
    }

    #[test]
    fn trait_unpin_type_path() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<TypePath>();
    }
}
