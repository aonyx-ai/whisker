use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// An order-sensitive hash of the inputs that shape a compiled crate
///
/// The build scripts of whisker-types and whisker-rust bake one of these
/// into each crate, and the custom lint plugin loader compares the plugin's
/// values against its own. The crate version cannot serve that comparison:
/// whisker's crates are unpublished and keep one version number between
/// releases, so two builds of the same version can compile different
/// source. The fingerprint detects that drift; it is not a security
/// measure, so an unkeyed 64-bit FNV-1a suffices and avoids pulling a
/// cryptography dependency into every build script.
///
/// Files enter the hash in sorted path order, with paths relative to the
/// added directory and `/` separators, so the fingerprint is identical
/// wherever the crate is checked out and on every platform. Every name
/// and every file's contents enter as their own run, carrying their
/// length, so no two trees whose bytes merely fall differently across
/// names and files can hash alike.
///
/// # Examples
///
/// ```
/// use whisker_codegen::Fingerprint;
///
/// let mut fingerprint = Fingerprint::new();
/// fingerprint.add_bytes(b"generated code");
///
/// assert_eq!(fingerprint.to_string().len(), 16);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Fingerprint(u64);

impl Fingerprint {
    /// Creates a fingerprint of nothing
    pub fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    /// Folds every file under `root` into the fingerprint
    ///
    /// Every file counts, whatever its extension: a file that turns out
    /// not to influence the build only makes the comparison stricter, and
    /// a stray mismatch is a refusal the user can see, while a missed one
    /// is undefined behavior nobody can.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] if a directory or file under `root` cannot be
    /// read.
    pub fn add_directory(&mut self, root: &Path) -> io::Result<()> {
        let mut files = Vec::new();
        collect_files(root, root, &mut files)?;
        files.sort();

        for (name, contents) in files {
            self.add_bytes(name.as_bytes());
            self.add_bytes(&contents);
        }

        Ok(())
    }

    /// Folds one run of bytes into the fingerprint
    ///
    /// This is how generated code, which lives outside the source tree,
    /// joins the hash.
    ///
    /// The run's length enters the hash ahead of its content, so a call
    /// never folds to the state some other split of the same bytes would
    /// reach: without that, a file named `a` holding `bc` and a file named
    /// `ab` holding `c` would be one byte stream and one fingerprint.
    pub fn add_bytes(&mut self, bytes: &[u8]) {
        self.fold(&(bytes.len() as u64).to_le_bytes());
        self.fold(bytes);
    }

    /// Folds bytes into the hash as they are
    fn fold(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }
}

impl Default for Fingerprint {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// Gathers `(relative path, contents)` pairs for the files under `dir`
fn collect_files(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();

        if path.is_dir() {
            collect_files(root, &path, files)?;
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .expect("collected paths should sit under the root")
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        files.push((relative, fs::read(&path)?));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn fingerprint_of(dir: &Path) -> String {
        let mut fingerprint = Fingerprint::new();
        fingerprint
            .add_directory(dir)
            .expect("should hash the directory");
        fingerprint.to_string()
    }

    #[test]
    fn add_bytes_changes_the_fingerprint() {
        let empty = Fingerprint::new();
        let mut fed = Fingerprint::new();

        fed.add_bytes(b"generated code");

        assert_ne!(empty, fed);
    }

    #[test]
    fn add_bytes_frames_each_run() {
        let mut split = Fingerprint::new();
        let mut whole = Fingerprint::new();

        split.add_bytes(b"ab");
        split.add_bytes(b"c");
        whole.add_bytes(b"abc");

        assert_ne!(split, whole);
    }

    #[test]
    fn add_directory_is_insensitive_to_creation_order() {
        let first = tempfile::tempdir().expect("should create a directory");
        fs::write(first.path().join("a.rs"), "a").expect("should write");
        fs::write(first.path().join("b.rs"), "b").expect("should write");
        let second = tempfile::tempdir().expect("should create a directory");
        fs::write(second.path().join("b.rs"), "b").expect("should write");
        fs::write(second.path().join("a.rs"), "a").expect("should write");

        assert_eq!(fingerprint_of(first.path()), fingerprint_of(second.path()));
    }

    #[test]
    fn add_directory_keeps_names_and_contents_apart() {
        let first = tempfile::tempdir().expect("should create a directory");
        fs::write(first.path().join("a"), "bc").expect("should write");
        let second = tempfile::tempdir().expect("should create a directory");
        fs::write(second.path().join("ab"), "c").expect("should write");

        assert_ne!(fingerprint_of(first.path()), fingerprint_of(second.path()));
    }

    #[test]
    fn add_directory_missing_root_returns_error() {
        let dir = tempfile::tempdir().expect("should create a directory");
        let missing = dir.path().join("absent");
        let mut fingerprint = Fingerprint::new();

        let error = fingerprint
            .add_directory(&missing)
            .expect_err("should fail");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn add_directory_sees_nested_files() {
        let flat = tempfile::tempdir().expect("should create a directory");
        fs::write(flat.path().join("a.rs"), "a").expect("should write");
        let nested = tempfile::tempdir().expect("should create a directory");
        fs::create_dir(nested.path().join("sub")).expect("should create");
        fs::write(nested.path().join("a.rs"), "a").expect("should write");
        fs::write(nested.path().join("sub").join("b.rs"), "b").expect("should write");

        assert_ne!(fingerprint_of(flat.path()), fingerprint_of(nested.path()));
    }

    #[test]
    fn different_contents_produce_different_fingerprints() {
        let first = tempfile::tempdir().expect("should create a directory");
        fs::write(first.path().join("a.rs"), "a").expect("should write");
        let second = tempfile::tempdir().expect("should create a directory");
        fs::write(second.path().join("a.rs"), "changed").expect("should write");

        assert_ne!(fingerprint_of(first.path()), fingerprint_of(second.path()));
    }

    #[test]
    fn display_renders_sixteen_hex_digits() {
        let fingerprint = Fingerprint::new();

        let rendered = fingerprint.to_string();

        assert_eq!(rendered.len(), 16);
        assert!(rendered.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn same_directory_fingerprints_identically() {
        let dir = tempfile::tempdir().expect("should create a directory");
        fs::write(dir.path().join("a.rs"), "a").expect("should write");

        assert_eq!(fingerprint_of(dir.path()), fingerprint_of(dir.path()));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Fingerprint>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Fingerprint>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Fingerprint>();
    }
}
