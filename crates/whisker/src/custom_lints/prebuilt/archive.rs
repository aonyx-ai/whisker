use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Component, Path};

use anyhow::Context as _;
use flate2::read::GzDecoder;
use sha2::{Digest as _, Sha256};

/// How many bytes of a file are hashed at a time
const CHUNK: usize = 64 * 1024;

/// The SHA-256 digest of a file whisker downloaded
///
/// A publisher writes the digest of each archive beside it, and whisker
/// compares the two before it unpacks anything. That catches a truncated
/// download, a corrupted one, and a stale one.
///
/// The pair proves that the bytes arrived intact. It does not establish
/// trust, because the same publisher writes both. Every library in the
/// archive still passes the plugin handshake.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    /// Returns the digest of the file at `path`
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn of_file(path: &Path) -> anyhow::Result<Self> {
        let file =
            File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; CHUNK];

        loop {
            let read = reader
                .read(&mut buffer)
                .with_context(|| format!("failed to read {}", path.display()))?;

            if read == 0 {
                break;
            }

            hasher.update(&buffer[..read]);
        }

        Ok(Self(hasher.finalize().into()))
    }

    /// Reads the digest that a sidecar file publishes
    ///
    /// This accepts the spelling `sha256sum` and `shasum` write, which is
    /// the digest, whitespace, and the file it describes. It also accepts
    /// a digest on its own, which is what a publisher writes by hand.
    ///
    /// # Errors
    ///
    /// Returns an error if the file holds no digest of the right shape.
    pub fn from_sidecar(contents: &str) -> anyhow::Result<Self> {
        let digest = contents
            .split_whitespace()
            .next()
            .context("the sidecar is empty")?;

        anyhow::ensure!(
            digest.len() == 64,
            "the sidecar holds {} characters where a SHA-256 digest has 64",
            digest.len()
        );

        let mut bytes = [0; 32];
        hex::decode_to_slice(digest.to_ascii_lowercase(), &mut bytes)
            .context("the sidecar holds something that is not a SHA-256 digest")?;

        Ok(Self(bytes))
    }
}

impl std::fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }

        Ok(())
    }
}

/// Unpacks the files an archive holds at its root into `directory`
///
/// A publisher ships one library per lint package, side by side, so
/// everything whisker needs sits at the root. Whisker skips every other
/// entry rather than refuse it. That leaves a publisher free to add a
/// manifest or a license, and whisker writes nothing it did not intend.
///
/// One rule keeps the unpack inside `directory`. A file at the root has
/// exactly one path component, which a name with `..` or a leading slash
/// fails. The rule also passes over a symbolic link, which is no regular
/// file, so an archive cannot plant one.
///
/// This does not bound how much a well-formed archive unpacks to. The
/// publisher already puts code in whisker's process, so a decompression
/// bomb adds nothing to what they can do.
///
/// # Errors
///
/// Returns an error if the archive cannot be read or a file cannot be
/// written.
pub fn extract(archive: &Path, directory: &Path) -> anyhow::Result<()> {
    let file =
        File::open(archive).with_context(|| format!("failed to open {}", archive.display()))?;
    let mut tar = tar::Archive::new(GzDecoder::new(BufReader::new(file)));

    let entries = tar
        .entries()
        .with_context(|| format!("failed to read {}", archive.display()))?;

    for entry in entries {
        let mut entry = entry.with_context(|| format!("failed to read {}", archive.display()))?;

        if entry.header().entry_type() != tar::EntryType::Regular {
            continue;
        }

        let path = entry
            .path()
            .with_context(|| format!("failed to read a file name from {}", archive.display()))?
            .into_owned();

        let Some(name) = root_file_name(&path) else {
            continue;
        };

        let destination = directory.join(name);
        entry
            .unpack(&destination)
            .with_context(|| format!("failed to write {}", destination.display()))?;
    }

    Ok(())
}

/// Returns the name of `path` if it names a file at an archive's root
///
/// Exactly one ordinary component qualifies. A path with a directory in
/// it fails, and so does an absolute path and one that climbs out with
/// `..`. The unpack therefore needs no separate check for the last two.
fn root_file_name(path: &Path) -> Option<&std::ffi::OsStr> {
    let mut components = path.components();

    let Some(Component::Normal(name)) = components.next() else {
        return None;
    };

    match components.next() {
        Some(_) => None,
        None => Some(name),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use tempfile::TempDir;

    use super::*;

    /// The digest of an empty input, from the SHA-256 specification
    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    /// Writes a gzipped tar holding `files`, and returns where it is
    ///
    /// This writes every name into the header as bytes rather than
    /// through the builder. The builder refuses a name that holds `..`,
    /// and several of these tests need exactly that archive.
    fn archive_of(directory: &Path, files: &[(&str, &[u8])]) -> std::path::PathBuf {
        archive_of_entries(
            directory,
            &files
                .iter()
                .map(|(name, contents)| (*name, *contents, tar::EntryType::Regular))
                .collect::<Vec<_>>(),
        )
    }

    /// Writes a gzipped tar holding `entries` of the kinds they name
    fn archive_of_entries(
        directory: &Path,
        entries: &[(&str, &[u8], tar::EntryType)],
    ) -> std::path::PathBuf {
        let path = directory.join("archive.tar.gz");
        let file = File::create(&path).expect("the archive should be created");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);

        for (name, contents, kind) in entries {
            let mut header = tar::Header::new_gnu();
            set_raw_name(&mut header, name);
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(*kind);
            header.set_cksum();
            builder
                .append(&header, *contents)
                .expect("the entry should be written");
        }

        builder
            .into_inner()
            .expect("the archive should be finished")
            .finish()
            .expect("the archive should be flushed");

        path
    }

    /// Writes `name` into `header` without the checks a builder makes
    ///
    /// No honest tool writes an archive that climbs out of its
    /// destination, so a test that proves whisker refuses one writes the
    /// bytes itself.
    fn set_raw_name(header: &mut tar::Header, name: &str) {
        let gnu = header.as_gnu_mut().expect("the header should be a GNU one");
        let bytes = name.as_bytes();

        assert!(bytes.len() < gnu.name.len(), "the name should fit: {name}");

        gnu.name[..bytes.len()].copy_from_slice(bytes);
    }

    /// Returns the names `extract` wrote into a fresh directory
    fn extracted(files: &[(&str, &[u8])]) -> Vec<String> {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let archive = archive_of(directory.path(), files);
        let into = directory.path().join("into");
        std::fs::create_dir(&into).expect("the destination should be created");

        extract(&archive, &into).expect("the archive should unpack");

        let mut names: Vec<String> = std::fs::read_dir(&into)
            .expect("the destination should be readable")
            .map(|entry| {
                entry
                    .expect("the entry should be readable")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();

        names
    }

    fn file(directory: &TempDir, name: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = directory.path().join(name);
        let mut file = File::create(&path).expect("the file should be created");
        file.write_all(contents)
            .expect("the file should be written");

        path
    }

    /// An archive must not write outside its destination. Such a name
    /// holds more than one component, so the rule that skips a
    /// subdirectory skips this too.
    #[test]
    fn extract_ignores_an_entry_that_climbs_out() {
        let names = extracted(&[("../escaped", b"no" as &[u8]), ("kept", b"yes")]);

        assert_eq!(names, vec!["kept".to_owned()]);
    }

    #[test]
    fn extract_ignores_an_absolute_entry() {
        let names = extracted(&[("/etc/passwd", b"no" as &[u8]), ("kept", b"yes")]);

        assert_eq!(names, vec!["kept".to_owned()]);
    }

    /// An archive must not be able to plant a link pointing anywhere.
    #[test]
    fn extract_ignores_a_symbolic_link() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let archive = archive_of_entries(
            directory.path(),
            &[
                ("link", b"" as &[u8], tar::EntryType::Symlink),
                ("kept", b"yes", tar::EntryType::Regular),
            ],
        );
        let into = directory.path().join("into");
        std::fs::create_dir(&into).expect("the destination should be created");

        extract(&archive, &into).expect("the archive should unpack");

        assert!(!into.join("link").exists());
        assert!(into.join("kept").is_file());
    }

    #[test]
    fn extract_ignores_an_entry_in_a_subdirectory() {
        let names = extracted(&[("nested/deep", b"no" as &[u8]), ("kept", b"yes")]);

        assert_eq!(names, vec!["kept".to_owned()]);
    }

    #[test]
    fn extract_of_an_empty_archive_writes_nothing() {
        let names = extracted(&[]);

        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn extract_writes_every_file_at_the_root() {
        let names = extracted(&[("first", b"one" as &[u8]), ("second", b"two")]);

        assert_eq!(names, vec!["first".to_owned(), "second".to_owned()]);
    }

    #[test]
    fn extract_writes_the_contents_it_was_given() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let archive = archive_of(directory.path(), &[("library", b"contents" as &[u8])]);
        let into = directory.path().join("into");
        std::fs::create_dir(&into).expect("the destination should be created");

        extract(&archive, &into).expect("the archive should unpack");

        let written = std::fs::read(into.join("library")).expect("the file should be readable");
        assert_eq!(written, b"contents");
    }

    #[test]
    fn from_sidecar_accepts_the_digest_alone() {
        let digest = Sha256Digest::from_sidecar(EMPTY).expect("should parse");

        assert_eq!(digest.to_string(), EMPTY);
    }

    #[test]
    fn from_sidecar_accepts_what_shasum_writes() {
        let digest =
            Sha256Digest::from_sidecar(&format!("{EMPTY}  rules.tar.gz\n")).expect("should parse");

        assert_eq!(digest.to_string(), EMPTY);
    }

    #[test]
    fn from_sidecar_accepts_uppercase() {
        let digest = Sha256Digest::from_sidecar(&EMPTY.to_ascii_uppercase()).expect("should parse");

        assert_eq!(digest.to_string(), EMPTY);
    }

    #[test]
    fn from_sidecar_of_an_empty_file_returns_error() {
        let error = Sha256Digest::from_sidecar("   \n").expect_err("should fail");

        assert!(format!("{error:#}").contains("empty"), "{error:#}");
    }

    #[test]
    fn from_sidecar_of_a_short_digest_returns_error() {
        let error = Sha256Digest::from_sidecar("abcdef").expect_err("should fail");

        assert!(format!("{error:#}").contains("64"), "{error:#}");
    }

    #[test]
    fn from_sidecar_of_something_that_is_not_hexadecimal_returns_error() {
        let error = Sha256Digest::from_sidecar(&"z".repeat(64)).expect_err("should fail");

        assert!(format!("{error:#}").contains("SHA-256"), "{error:#}");
    }

    #[test]
    fn of_file_matches_the_published_digest_of_no_bytes() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = file(&directory, "empty", b"");

        let digest = Sha256Digest::of_file(&path).expect("the file should be read");

        assert_eq!(digest.to_string(), EMPTY);
    }

    /// Pins that the digest covers a file larger than one buffer
    #[test]
    fn of_file_reads_a_file_larger_than_one_chunk() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let contents = vec![7; CHUNK * 2 + 1];
        let path = file(&directory, "large", &contents);

        let digest = Sha256Digest::of_file(&path).expect("the file should be read");

        let mut hasher = Sha256::new();
        hasher.update(&contents);
        assert_eq!(digest, Sha256Digest(hasher.finalize().into()));
    }

    #[test]
    fn of_file_separates_different_contents() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let first = file(&directory, "first", b"one");
        let second = file(&directory, "second", b"two");

        assert_ne!(
            Sha256Digest::of_file(&first).expect("the file should be read"),
            Sha256Digest::of_file(&second).expect("the file should be read")
        );
    }

    #[test]
    fn of_a_missing_file_returns_error() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        let error =
            Sha256Digest::of_file(&directory.path().join("absent")).expect_err("should fail");

        assert!(format!("{error:#}").contains("absent"), "{error:#}");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Sha256Digest>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Sha256Digest>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Sha256Digest>();
    }
}
