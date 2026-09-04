use std::fmt;

use whisker_rust::plugin;

use super::digest::digest;
use super::handshake::AbiIdentity;

/// The target triple this binary was compiled for
///
/// Nothing at runtime reports it, so the build script reads it from cargo
/// and bakes it in.
const TARGET: &str = env!("WHISKER_TARGET");

/// Names the whisker binary that a prebuilt lint library has to fit
///
/// The tag holds a digest of every value [`super::handshake`] compares,
/// then the platform. A publisher of prebuilt lints puts it in the name
/// of each archive. Whisker can therefore ask for a library that fits
/// before it downloads one.
///
/// The tag covers what the handshake covers. An archive under this
/// whisker's tag passes the handshake, and a whisker that no publisher
/// built for finds no file at all.
///
/// A small digest suffices here. The handshake still decides whether a
/// library loads, so a collision costs one wasted download.
///
/// The name is a contract with publishers, and [`AbiTag::new`] carries a
/// test that pins it.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AbiTag(String);

impl AbiTag {
    /// Returns the tag of the whisker binary that is running
    ///
    /// # Examples
    ///
    /// ```ignore
    /// println!("this whisker loads lints tagged {}", AbiTag::host());
    /// ```
    pub fn host() -> Self {
        Self::new(&AbiIdentity::host(), TARGET)
    }

    /// Returns the tag of a whisker with `identity` running on `target`
    ///
    /// A newline separates the values, and each fingerprint occupies a
    /// fixed width. Two different identities therefore cannot produce one
    /// input to the digest.
    pub(super) fn new(identity: &AbiIdentity, target: &str) -> Self {
        let AbiIdentity {
            abi_version: _,
            rustc_version,
            types_fingerprint,
            language_fingerprint,
        } = identity;

        // The floor rather than the version whisker writes. Whisker loads
        // every protocol from the floor upward, so two whiskers sharing
        // one accept each other's archives, and raising the version alone
        // does not strand what a publisher already built. Raising the
        // floor does, which is the point: that is when older plugins stop
        // loading.
        let floor = plugin::MIN_ABI_VERSION;

        let key = digest(&format!(
            "{floor}\n{rustc_version}\n{types_fingerprint:016x}\n{language_fingerprint:016x}"
        ));

        Self(format!("{key}-{target}"))
    }
}

impl fmt::Display for AbiTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> AbiIdentity {
        AbiIdentity {
            abi_version: 2,
            rustc_version: "rustc 1.92.0-nightly (0123456 2026-08-11)".to_owned(),
            types_fingerprint: 0x0123_4567_89ab_cdef,
            language_fingerprint: 0xfedc_ba98_7654_3210,
        }
    }

    /// Pins the tag that publishers write into an archive name
    ///
    /// Whisker stops finding every archive that carries the old tag if
    /// this derivation changes. The test fails first, so whoever changes
    /// it knows to republish.
    #[test]
    fn new_is_stable_across_releases() {
        let tag = AbiTag::new(&identity(), "aarch64-apple-darwin");

        assert_eq!(tag.to_string(), "4d1b722e64837210-aarch64-apple-darwin");
    }

    /// A protocol version is not part of the tag
    ///
    /// Whisker loads every protocol from the floor upward, so two
    /// whiskers that share a floor accept each other's archives. Raising
    /// the version alone must therefore not strand what a publisher has
    /// already built; raising the floor does, which is what the floor is
    /// for.
    #[test]
    fn new_ignores_the_protocol_a_whisker_writes() {
        let other = AbiIdentity {
            abi_version: identity().abi_version + 1,
            ..identity()
        };

        assert_eq!(
            AbiTag::new(&identity(), "x86_64-unknown-linux-gnu"),
            AbiTag::new(&other, "x86_64-unknown-linux-gnu")
        );
    }

    #[test]
    fn new_separates_identities_that_differ_in_the_language_fingerprint() {
        let other = AbiIdentity {
            language_fingerprint: 1,
            ..identity()
        };

        assert_ne!(
            AbiTag::new(&identity(), "x86_64-unknown-linux-gnu"),
            AbiTag::new(&other, "x86_64-unknown-linux-gnu")
        );
    }

    #[test]
    fn new_separates_identities_that_differ_in_the_rustc_version() {
        let other = AbiIdentity {
            rustc_version: "rustc 1.92.0-nightly (0123456 2026-08-12)".to_owned(),
            ..identity()
        };

        assert_ne!(
            AbiTag::new(&identity(), "x86_64-unknown-linux-gnu"),
            AbiTag::new(&other, "x86_64-unknown-linux-gnu")
        );
    }

    #[test]
    fn new_separates_identities_that_differ_in_the_types_fingerprint() {
        let other = AbiIdentity {
            types_fingerprint: 1,
            ..identity()
        };

        assert_ne!(
            AbiTag::new(&identity(), "x86_64-unknown-linux-gnu"),
            AbiTag::new(&other, "x86_64-unknown-linux-gnu")
        );
    }

    #[test]
    fn new_separates_platforms() {
        assert_ne!(
            AbiTag::new(&identity(), "aarch64-apple-darwin"),
            AbiTag::new(&identity(), "x86_64-unknown-linux-gnu")
        );
    }

    /// Pins that one fingerprint cannot borrow a digit from the other
    ///
    /// At a variable width the two would run together. Two different
    /// identities would then share one tag.
    #[test]
    fn new_separates_identities_whose_fingerprints_are_shifted() {
        let first = AbiIdentity {
            types_fingerprint: 0x0000_0000_0000_0001,
            language_fingerprint: 0x0000_0000_0000_0023,
            ..identity()
        };
        let second = AbiIdentity {
            types_fingerprint: 0x0000_0000_0000_0012,
            language_fingerprint: 0x0000_0000_0000_0003,
            ..identity()
        };

        assert_ne!(
            AbiTag::new(&first, "aarch64-apple-darwin"),
            AbiTag::new(&second, "aarch64-apple-darwin")
        );
    }

    #[test]
    fn host_names_the_platform_the_binary_was_built_for() {
        let tag = AbiTag::host().to_string();

        assert!(tag.ends_with(&format!("-{TARGET}")), "{tag}");
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AbiTag>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AbiTag>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AbiTag>();
    }
}
