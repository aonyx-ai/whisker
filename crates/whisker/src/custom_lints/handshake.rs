use std::fmt;

use whisker_rust::plugin;

/// The ABI-relevant identity of one side of the plugin boundary
///
/// Rust has no stable ABI, so a plugin and the whisker binary only agree on
/// the layout of the types that cross between them when the same rustc
/// compiled both from the same whisker source. This type captures exactly
/// those facts for one side. [`validate`] compares the two sides.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct AbiIdentity {
    pub abi_version: u32,
    pub rustc_version: String,
    pub types_fingerprint: String,
    pub language_fingerprint: String,
}

impl AbiIdentity {
    /// Returns the identity baked into this whisker binary
    pub fn host() -> Self {
        Self {
            abi_version: plugin::ABI_VERSION,
            rustc_version: plugin::RUSTC_VERSION.to_string_lossy().into_owned(),
            types_fingerprint: plugin::TYPES_FINGERPRINT.to_string_lossy().into_owned(),
            language_fingerprint: plugin::LANGUAGE_FINGERPRINT.to_string_lossy().into_owned(),
        }
    }
}

/// Accepts a plugin's declaration protocol version
///
/// This is the first check of the handshake and the only one a plugin of
/// any vintage can be held to, because the version sits at offset zero of
/// a `#[repr(C)]` struct whose remaining fields a mismatching plugin may
/// have laid out differently. The caller reads it through a raw pointer
/// and calls this before taking a reference to the declaration.
///
/// The version takes a bare integer rather than an [`AbiIdentity`] for
/// that reason: at the point of this check, the rest of the plugin's
/// identity has not been read yet.
///
/// # Errors
///
/// Returns [`HandshakeMismatch::AbiVersion`] if the versions differ.
pub fn validate_abi_version(host: &AbiIdentity, plugin: u32) -> Result<(), HandshakeMismatch> {
    if host.abi_version != plugin {
        return Err(HandshakeMismatch::AbiVersion {
            host: host.abi_version,
            plugin,
        });
    }

    Ok(())
}

/// Accepts a plugin only when the rest of its identity matches the host's
///
/// The checks run in the order the declaration's fields become
/// trustworthy, and the first mismatch wins, so the reported error is the
/// one whose remedy applies. The protocol version is not among them:
/// [`validate_abi_version`] settles it before the fields this function
/// compares may be read at all, so repeating it here would be a check
/// whose failure no caller can reach.
///
/// # Errors
///
/// Returns the first [`HandshakeMismatch`] between the two identities.
pub fn validate_identity(
    host: &AbiIdentity,
    plugin: &AbiIdentity,
) -> Result<(), HandshakeMismatch> {
    if host.rustc_version != plugin.rustc_version {
        return Err(HandshakeMismatch::RustcVersion {
            host: host.rustc_version.clone(),
            plugin: plugin.rustc_version.clone(),
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
/// Every variant is a refusal to load, because each one means the two
/// images cannot be assumed to agree on type layout, and a wrong
/// assumption is undefined behavior rather than a wrong answer. The
/// fingerprint variants carry no values: the hashes mean nothing to a
/// reader, while the version strings in the other variants tell the user
/// what to install.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum HandshakeMismatch {
    AbiVersion { host: u32, plugin: u32 },
    RustcVersion { host: String, plugin: String },
    TypesFingerprint,
    LanguageFingerprint,
}

impl fmt::Display for HandshakeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandshakeMismatch::AbiVersion { host, plugin } => write!(
                f,
                "the plugin speaks declaration protocol version {plugin}, but this whisker \
                 speaks version {host}; rebuild the plugin against the whisker revision this \
                 binary was built from"
            ),
            HandshakeMismatch::RustcVersion { host, plugin } => write!(
                f,
                "the plugin was built by `{plugin}`, but whisker was built by `{host}`; build \
                 the plugin with the same toolchain"
            ),
            HandshakeMismatch::TypesFingerprint => write!(
                f,
                "the plugin was built against a different revision of whisker-types; update the \
                 plugin's whisker dependencies to the revision whisker was built from"
            ),
            HandshakeMismatch::LanguageFingerprint => write!(
                f,
                "the plugin was built against a different revision of whisker-rust; update the \
                 plugin's whisker dependencies to the revision whisker was built from"
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
            abi_version: 1,
            rustc_version: "rustc 1.92.0-nightly (0000000 2026-08-11)".into(),
            types_fingerprint: "00000000000000aa".into(),
            language_fingerprint: "00000000000000bb".into(),
        }
    }

    #[test]
    fn host_reports_the_baked_in_constants() {
        let host = AbiIdentity::host();

        assert_eq!(host.abi_version, plugin::ABI_VERSION);
        assert!(host.rustc_version.starts_with("rustc"));
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
    fn validate_abi_version_with_a_differing_version_returns_a_mismatch() {
        let error = validate_abi_version(&identity(), 2).expect_err("should mismatch");

        assert_eq!(error, HandshakeMismatch::AbiVersion { host: 1, plugin: 2 });
        assert!(error.to_string().contains("rebuild the plugin"));
    }

    #[test]
    fn validate_abi_version_with_the_host_version_succeeds() {
        let host = identity();

        validate_abi_version(&host, host.abi_version).expect("should match");
    }

    #[test]
    fn validate_identity_with_a_differing_language_fingerprint_returns_a_mismatch() {
        let mut plugin = identity();
        plugin.language_fingerprint = "00000000000000cc".into();

        let error = validate_identity(&identity(), &plugin).expect_err("should mismatch");

        assert_eq!(error, HandshakeMismatch::LanguageFingerprint);
        assert!(error.to_string().contains("whisker-rust"));
    }

    #[test]
    fn validate_identity_with_a_differing_rustc_version_returns_a_mismatch() {
        let mut plugin = identity();
        plugin.rustc_version = "rustc 1.93.0-nightly (1111111 2026-09-01)".into();

        let error = validate_identity(&identity(), &plugin).expect_err("should mismatch");

        let message = error.to_string();
        assert!(message.contains("1.92.0"), "should name both: {message}");
        assert!(message.contains("1.93.0"), "should name both: {message}");
        assert!(message.contains("same toolchain"), "unexpected: {message}");
    }

    #[test]
    fn validate_identity_with_a_differing_types_fingerprint_returns_a_mismatch() {
        let mut plugin = identity();
        plugin.types_fingerprint = "00000000000000cc".into();

        let error = validate_identity(&identity(), &plugin).expect_err("should mismatch");

        assert_eq!(error, HandshakeMismatch::TypesFingerprint);
        assert!(error.to_string().contains("whisker-types"));
    }

    #[test]
    fn validate_identity_with_matching_identities_succeeds() {
        validate_identity(&identity(), &identity()).expect("should match");
    }
}
