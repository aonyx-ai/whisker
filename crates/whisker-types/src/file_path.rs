use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use stabby::sync::ArcSlice;

/// The path of a source file, shared by every span that points into it
///
/// A [`Span`] names its file through one of these, and a span crosses the
/// plugin boundary. Std promises nothing about an `Arc`'s layout from one
/// compiler to the next, so the path sits behind a reference count that
/// stabby lays out. A plugin built by one rustc clones and drops a path
/// that a whisker built by another allocated. A clone copies two pointers
/// and bumps the count; it copies no path bytes.
///
/// On Unix the path is kept as the bytes the operating system gave, so a
/// name that is not UTF-8 comes back unchanged. Elsewhere std promises no
/// encoding for such a name that holds across compilers. There the path
/// is kept as UTF-8, and a name that is not is converted the way
/// [`Path::to_string_lossy`] converts it.
///
/// The type dereferences to [`Path`] and converts from anything that is
/// [`AsRef<Path>`], so a [`PathBuf`] or a string literal builds one. It
/// does not implement [`AsRef<Path>`] itself, because that conversion
/// would then overlap with the reflexive one; use [`FilePath::as_path`]
/// where a `&Path` is required.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// use whisker_types::FilePath;
///
/// let file = FilePath::from("src/lib.rs");
///
/// assert_eq!(file.as_path(), Path::new("src/lib.rs"));
/// assert_eq!(file.clone(), file);
/// ```
///
/// [`AsRef<Path>`]: std::convert::AsRef
/// [`Path::to_string_lossy`]: std::path::Path::to_string_lossy
/// [`PathBuf`]: std::path::PathBuf
/// [`Span`]: crate::Span
#[stabby::stabby]
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct FilePath(ArcSlice<u8>);

impl FilePath {
    /// Returns the path this holds
    ///
    /// # Panics
    ///
    /// Panics on a platform other than Unix if the bytes are not UTF-8.
    /// Every conversion that builds a `FilePath` on such a platform writes
    /// UTF-8.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// use whisker_types::FilePath;
    ///
    /// let file = FilePath::from(PathBuf::from("src/lib.rs"));
    ///
    /// assert_eq!(file.as_path(), Path::new("src/lib.rs"));
    /// ```
    pub fn as_path(&self) -> &Path {
        decode(self.0.as_slice())
    }
}

impl Deref for FilePath {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.as_path()
    }
}

impl<P: AsRef<Path>> From<P> for FilePath {
    fn from(path: P) -> Self {
        Self(ArcSlice::from(encode(path.as_ref()).as_ref()))
    }
}

impl fmt::Debug for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_path().fmt(f)
    }
}

/// Returns the bytes that stand for `path`
#[cfg(unix)]
fn encode(path: &Path) -> Cow<'_, [u8]> {
    Cow::Borrowed(path.as_os_str().as_bytes())
}

/// Returns the bytes that stand for `path`
#[cfg(not(unix))]
fn encode(path: &Path) -> Cow<'_, [u8]> {
    match path.to_string_lossy() {
        Cow::Borrowed(text) => Cow::Borrowed(text.as_bytes()),
        Cow::Owned(text) => Cow::Owned(text.into_bytes()),
    }
}

/// Returns the path that `bytes` stand for
#[cfg(unix)]
fn decode(bytes: &[u8]) -> &Path {
    Path::new(std::ffi::OsStr::from_bytes(bytes))
}

/// Returns the path that `bytes` stand for
#[cfg(not(unix))]
fn decode(bytes: &[u8]) -> &Path {
    Path::new(std::str::from_utf8(bytes).expect("a FilePath holds UTF-8 on this platform"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn as_path_roundtrips_a_path() {
        let file = FilePath::from(PathBuf::from("src/lib.rs"));

        let path = file.as_path();

        assert_eq!(path, Path::new("src/lib.rs"));
    }

    #[cfg(unix)]
    #[test]
    fn as_path_roundtrips_bytes_that_are_not_utf8() {
        let name = OsStr::from_bytes(b"src/caf\xe9.rs");
        let file = FilePath::from(Path::new(name));

        let path = file.as_path();

        assert_eq!(path.as_os_str(), name);
    }

    #[test]
    fn clone_shares_the_bytes() {
        let file = FilePath::from("src/lib.rs");

        let copy = file.clone();

        assert_eq!(ArcSlice::strong_count(&file.0), 2);
        assert_eq!(copy, file);
    }

    #[test]
    fn debug_shows_the_path() {
        let file = FilePath::from("src/lib.rs");

        let text = format!("{file:?}");

        assert_eq!(text, "\"src/lib.rs\"");
    }

    #[test]
    fn deref_reaches_path_methods() {
        let file = FilePath::from("src/lib.rs");

        let name = file.file_name();

        assert_eq!(name, Some(OsStr::new("lib.rs")));
    }

    #[test]
    fn eq_compares_by_content() {
        let literal = FilePath::from("a.rs");
        let owned = FilePath::from(PathBuf::from("a.rs"));
        let other = FilePath::from("b.rs");

        assert_eq!(literal, owned);
        assert_ne!(literal, other);
    }

    #[test]
    fn hash_agrees_with_eq() {
        let mut files = HashSet::new();
        files.insert(FilePath::from("a.rs"));

        let found = files.contains(&FilePath::from(PathBuf::from("a.rs")));

        assert!(found);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FilePath>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FilePath>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<FilePath>();
    }

    #[cfg(unix)]
    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            #[test]
            fn as_path_roundtrips_any_bytes(
                bytes in proptest::collection::vec(any::<u8>(), 0..64),
            ) {
                let name = OsStr::from_bytes(&bytes);

                let file = FilePath::from(Path::new(name));

                prop_assert_eq!(file.as_path().as_os_str(), name);
            }

            #[test]
            fn from_a_str_equals_from_its_path_buf(name in "[a-z/]{1,20}\\.rs") {
                let literal = FilePath::from(name.as_str());

                let owned = FilePath::from(PathBuf::from(&name));

                prop_assert_eq!(literal, owned);
            }
        }
    }
}
