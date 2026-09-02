use std::fmt;

use crate::config::GitRev;
use crate::custom_lints::AbiTag;

/// The extension every archive of prebuilt lints carries
const EXTENSION: &str = ".tar.gz";

/// The extension of the file that publishes an archive's digest
const SIDECAR: &str = ".sha256";

/// What an archive of prebuilt lints is called on a release
///
/// The name answers two questions about an archive: which commit of the
/// lints it holds, and which whisker it fits. A publisher writes the
/// name and whisker reads it, and neither program asks the other, so the
/// spelling here is a contract between them.
///
/// The commit comes first, because its width never varies. The
/// [`AbiTag`] can then hold the dashes that every target triple has.
///
/// # Examples
///
/// ```ignore
/// let name = AssetName::new(source.rev(), &AbiTag::host());
///
/// println!("looking for {name} and {}", name.sidecar());
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AssetName(String);

impl AssetName {
    /// Returns the name of the archive holding `rev` built for `tag`
    pub fn new(rev: &GitRev, tag: &AbiTag) -> Self {
        Self(format!("{rev}-{tag}{EXTENSION}"))
    }

    /// Returns the name as a publisher would have written it
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the name of the file that publishes this archive's digest
    pub fn sidecar(&self) -> String {
        format!("{}{SIDECAR}", self.0)
    }
}

impl fmt::Display for AssetName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_lints::handshake::AbiIdentity;

    const REV: &str = "0123456789abcdef0123456789abcdef01234567";

    fn tag() -> AbiTag {
        AbiTag::new(
            &AbiIdentity {
                abi_version: 2,
                rustc_version: "rustc 1.92.0-nightly (0123456 2026-08-11)".to_owned(),
                types_fingerprint: 0,
                language_fingerprint: 0,
            },
            "aarch64-apple-darwin",
        )
    }

    fn name() -> AssetName {
        AssetName::new(
            &GitRev::new(REV).expect("the revision should be accepted"),
            &tag(),
        )
    }

    #[test]
    fn new_ends_with_the_archive_extension() {
        assert!(name().as_str().ends_with(".tar.gz"), "{}", name());
    }

    #[test]
    fn new_names_the_commit_first() {
        assert!(name().as_str().starts_with(REV), "{}", name());
    }

    /// The tag holds dashes, and so does the boundary before it. The
    /// commit separates the two, because its width never varies.
    #[test]
    fn new_names_the_tag_after_the_commit() {
        let name = name();

        let rest = name
            .as_str()
            .strip_prefix(REV)
            .and_then(|rest| rest.strip_prefix('-'))
            .expect("the commit should come first");

        assert_eq!(rest, format!("{}.tar.gz", tag()));
    }

    #[test]
    fn sidecar_appends_to_the_archive_name() {
        let name = name();

        assert_eq!(name.sidecar(), format!("{name}.sha256"));
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AssetName>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AssetName>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AssetName>();
    }
}
