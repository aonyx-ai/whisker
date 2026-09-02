//! A short, stable digest for names that outlive the process
//!
//! Whisker writes digests into the names of directories it keeps, and
//! into the names of artifacts it looks for. Both outlive the run that
//! wrote them. A later run, a later release, and another platform all
//! reproduce them byte for byte.
//!
//! That rules out [`DefaultHasher`], which promises nothing across
//! releases: a toolchain upgrade that silently moved every cached
//! checkout would refetch the world. The algorithm is therefore spelled
//! out here. It is FNV-1a, which is short enough to read inside a path
//! and simple enough that the definition cannot drift.
//!
//! The digest distinguishes inputs; it does not authenticate them, and
//! nothing here asks it to.
//!
//! [`DefaultHasher`]: std::hash::DefaultHasher

/// Returns a short, stable digest of `text`
///
/// # Examples
///
/// ```ignore
/// assert_eq!(digest("https://example.com/rules"), "cc3eedebbb64629b");
/// ```
pub fn digest(text: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }

    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_is_stable_across_releases() {
        assert_eq!(digest("https://example.com/rules"), "cc3eedebbb64629b");
    }

    #[test]
    fn digest_is_sixteen_hexadecimal_digits() {
        let digest = digest("whatever");

        assert_eq!(digest.len(), 16, "{digest}");
        assert!(
            digest
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
            "{digest}"
        );
    }

    #[test]
    fn digest_separates_different_inputs() {
        assert_ne!(
            digest("https://example.com/a/rules"),
            digest("https://example.com/b/rules")
        );
    }
}
