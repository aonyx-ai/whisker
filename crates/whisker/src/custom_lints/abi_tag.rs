use std::fmt;

use super::digest::digest;
use super::handshake::AbiIdentity;

/// The target triple this binary was compiled for
///
/// Nothing at runtime reports it, so the build script reads it from cargo
/// and bakes it in.
const TARGET: &str = env!("WHISKER_TARGET");

/// Names the whisker binary that a prebuilt lint library has to fit
///
/// The tag holds a digest of the two fingerprints [`super::handshake`]
/// compares and the floor of the protocol it reads, then the platform. A
/// publisher of prebuilt lints puts it in the name of each archive.
/// Whisker can therefore ask for a library that fits before it downloads
/// one.
///
/// An archive under this whisker's tag passes the handshake. One case
/// escapes that. A later whisker built the archive, and the protocol
/// grew in between. The tag carries the floor, and the handshake refuses
/// a plugin newer than the whisker that reads it. Before 1.0 the floor
/// is the whole version, so a tag names one protocol exactly.
///
/// A whisker that no publisher built for finds no file at all. The
/// compiler is not among the inputs. A library built by any rustc fits,
/// so one archive serves every whisker built from the same boundary.
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
            abi_version,
            types_fingerprint,
            language_fingerprint,
        } = identity;

        // The floor rather than the version whisker writes. Whisker reads
        // every protocol from the floor upward, so two whiskers sharing
        // one accept each other's archives. From 1.0 a minor therefore
        // does not strand what a publisher already built, and a major
        // does, which is the point: that is when older plugins stop
        // loading. Before 1.0 the floor is the version itself, so every
        // release asks for its own archives.
        let floor = abi_version.floor();

        let key = digest(&format!(
            "{floor}\n{types_fingerprint:016x}\n{language_fingerprint:016x}"
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
    use whisker_rust::plugin::AbiVersion;

    use super::*;

    /// An identity from after 1.0, where a minor and a major differ in
    /// what they strand
    fn identity() -> AbiIdentity {
        AbiIdentity {
            abi_version: AbiVersion { major: 1, minor: 4 },
            types_fingerprint: 0x0123_4567_89ab_cdef,
            language_fingerprint: 0xfedc_ba98_7654_3210,
        }
    }

    /// Pins the tag that publishers write into an archive name
    ///
    /// Whisker stops finding every archive that carries the old tag if
    /// this derivation changes. The test fails first, so whoever changes
    /// it knows to republish. The floor is one of the inputs, so the
    /// value here moves with the one version change that is meant to
    /// strand what publishers built.
    #[test]
    fn new_is_stable_across_releases() {
        let tag = AbiTag::new(&identity(), "aarch64-apple-darwin");

        assert_eq!(tag.to_string(), "b7d58f06e26e0a22-aarch64-apple-darwin");
    }

    /// From 1.0 a minor is not part of the tag
    ///
    /// Whisker reads every protocol from the floor upward, so two
    /// whiskers that share a major accept each other's archives. A minor
    /// must therefore not strand what a publisher has already built; a
    /// major does, which is what the major is for.
    #[test]
    fn new_ignores_a_minor_from_one_point_zero_onward() {
        let other = AbiIdentity {
            abi_version: AbiVersion { major: 1, minor: 9 },
            ..identity()
        };

        assert_eq!(
            AbiTag::new(&identity(), "x86_64-unknown-linux-gnu"),
            AbiTag::new(&other, "x86_64-unknown-linux-gnu")
        );
    }

    /// Before 1.0 a minor is part of the tag
    ///
    /// Nothing is promised there, so a plugin loads only on the whisker
    /// it was built for and each release asks for archives of its own.
    #[test]
    fn new_separates_minors_before_one_point_zero() {
        let first = AbiIdentity {
            abi_version: AbiVersion { major: 0, minor: 1 },
            ..identity()
        };
        let second = AbiIdentity {
            abi_version: AbiVersion { major: 0, minor: 2 },
            ..identity()
        };

        assert_ne!(
            AbiTag::new(&first, "x86_64-unknown-linux-gnu"),
            AbiTag::new(&second, "x86_64-unknown-linux-gnu")
        );
    }

    #[test]
    fn new_separates_identities_that_differ_in_the_major() {
        let other = AbiIdentity {
            abi_version: AbiVersion { major: 2, minor: 4 },
            ..identity()
        };

        assert_ne!(
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
