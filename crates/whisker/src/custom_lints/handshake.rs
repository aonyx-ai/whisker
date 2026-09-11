use std::fmt;

use whisker_rust::plugin::{self, AbiVersion};

/// The ABI-relevant identity of one side of the plugin boundary
///
/// Rust has no stable ABI of its own, so stabby lays out every type that
/// crosses the plugin boundary. A plugin and the whisker binary agree on
/// those layouts when the whisker source each was built from lays them
/// out the same way. Which rustc built each side does not enter into it.
/// This type holds those facts for one side, and [`validate`] compares
/// the two sides.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct AbiIdentity {
    pub abi_version: AbiVersion,
    pub types_fingerprint: u64,
    pub language_fingerprint: u64,
}

impl AbiIdentity {
    /// Returns the identity baked into this whisker binary
    pub fn host() -> Self {
        Self {
            abi_version: plugin::ABI_VERSION,
            types_fingerprint: plugin::TYPES_FINGERPRINT,
            language_fingerprint: plugin::LANGUAGE_FINGERPRINT,
        }
    }
}

/// Accepts a plugin only when its identity matches the host's
///
/// The checks run in the order the declaration's fields become
/// trustworthy, and the first mismatch wins, so the reported error is the
/// one whose remedy applies.
///
/// # Errors
///
/// Returns the first [`HandshakeMismatch`] between the two identities.
pub fn validate(host: &AbiIdentity, plugin: &AbiIdentity) -> Result<(), HandshakeMismatch> {
    if !host.abi_version.accepts(plugin.abi_version) {
        return Err(HandshakeMismatch::AbiVersion {
            plugin: plugin.abi_version,
            host: host.abi_version,
        });
    }

    if host.types_fingerprint != plugin.types_fingerprint {
        return Err(HandshakeMismatch::TypesFingerprint);
    }

    if host.language_fingerprint != plugin.language_fingerprint {
        return Err(HandshakeMismatch::LanguageFingerprint);
    }

    Ok(())
}

/// A difference between the plugin's ABI identity and the host's
///
/// Every variant is a refusal to load: the two images may lay a type out
/// differently, and acting on that difference is undefined behavior. The
/// fingerprint variants carry no values: the hashes mean nothing to a
/// reader, while the versions in the other variant tell the user which
/// side to rebuild.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum HandshakeMismatch {
    AbiVersion {
        plugin: AbiVersion,
        host: AbiVersion,
    },
    TypesFingerprint,
    LanguageFingerprint,
}

impl fmt::Display for HandshakeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandshakeMismatch::AbiVersion { plugin, host } => {
                let floor = host.floor();
                let accepted = match floor == *host {
                    true => format!("{host}"),
                    false => format!("{floor}-{host}"),
                };

                write!(
                    f,
                    "plugin (ABI {plugin}) incompatible with whisker (ABI {accepted})"
                )
            }
            HandshakeMismatch::TypesFingerprint => write!(
                f,
                "plugin built against another whisker-types; match its whisker pin"
            ),
            HandshakeMismatch::LanguageFingerprint => write!(
                f,
                "plugin built against another whisker-rust; match its whisker pin"
            ),
        }
    }
}

impl std::error::Error for HandshakeMismatch {}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> AbiIdentity {
        AbiIdentity {
            abi_version: plugin::ABI_VERSION,
            types_fingerprint: 0xaa,
            language_fingerprint: 0xbb,
        }
    }

    /// Returns a version no whisker at [`plugin::ABI_VERSION`] reads
    ///
    /// A later major is refused in either era, so this holds whatever
    /// the constant moves to.
    fn unreadable() -> AbiVersion {
        AbiVersion {
            major: plugin::ABI_VERSION.major + 1,
            minor: 0,
        }
    }

    #[test]
    fn host_reports_the_baked_in_constants() {
        let host = AbiIdentity::host();

        assert_eq!(host.abi_version, plugin::ABI_VERSION);
        assert_eq!(host.types_fingerprint, plugin::TYPES_FINGERPRINT);
        assert_eq!(host.language_fingerprint, plugin::LANGUAGE_FINGERPRINT);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AbiIdentity>();
        assert_send::<HandshakeMismatch>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AbiIdentity>();
        assert_sync::<HandshakeMismatch>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<AbiIdentity>();
        assert_unpin::<HandshakeMismatch>();
    }

    #[test]
    fn validate_matching_identities_succeeds() {
        validate(&identity(), &identity()).expect("should match");
    }

    /// A version outside the supported range is reported before anything
    /// else, because the fields the rest reads are only readable once the
    /// declaration's shape is known.
    #[test]
    fn validate_reports_an_unsupported_abi_version_first() {
        let mut plugin = identity();
        plugin.abi_version = unreadable();
        plugin.types_fingerprint = 0xcc;

        let error = validate(&identity(), &plugin).expect_err("should mismatch");

        assert_eq!(
            error,
            HandshakeMismatch::AbiVersion {
                plugin: unreadable(),
                host: plugin::ABI_VERSION,
            }
        );
    }

    /// The oldest protocol whisker reads is accepted. From 1.0 that is an
    /// older minor, whose declaration ends sooner in a shape whisker
    /// knows; before 1.0 it is this version itself.
    #[test]
    fn validate_accepts_the_oldest_supported_abi_version() {
        let mut plugin = identity();
        plugin.abi_version = plugin::ABI_VERSION.floor();

        validate(&identity(), &plugin).expect("should accept");
    }

    /// The host side names every protocol whisker reads, not just the
    /// newest, because a plugin between the two is accepted and a reader
    /// who saw only the newest would rebuild for no reason.
    #[test]
    fn an_abi_mismatch_names_the_range_whisker_reads() {
        let error = HandshakeMismatch::AbiVersion {
            plugin: AbiVersion { major: 0, minor: 9 },
            host: AbiVersion { major: 1, minor: 2 },
        };

        assert_eq!(
            error.to_string(),
            "plugin (ABI 0.9) incompatible with whisker (ABI 1.0-1.2)"
        );
    }

    /// A whisker that reads one protocol names one, not a range of one.
    /// Every whisker before 1.0 is such a whisker.
    #[test]
    fn an_abi_mismatch_names_one_version_when_that_is_all_whisker_reads() {
        let error = HandshakeMismatch::AbiVersion {
            plugin: AbiVersion { major: 0, minor: 2 },
            host: AbiVersion { major: 0, minor: 1 },
        };

        assert_eq!(
            error.to_string(),
            "plugin (ABI 0.2) incompatible with whisker (ABI 0.1)"
        );
    }

    #[test]
    fn validate_reports_a_differing_language_fingerprint() {
        let mut plugin = identity();
        plugin.language_fingerprint = 0xcc;

        let error = validate(&identity(), &plugin).expect_err("should mismatch");

        assert_eq!(error, HandshakeMismatch::LanguageFingerprint);
        assert!(error.to_string().contains("whisker-rust"));
    }

    #[test]
    fn validate_reports_a_differing_types_fingerprint() {
        let mut plugin = identity();
        plugin.types_fingerprint = 0xcc;

        let error = validate(&identity(), &plugin).expect_err("should mismatch");

        assert_eq!(error, HandshakeMismatch::TypesFingerprint);
        assert!(error.to_string().contains("whisker-types"));
    }
}
