/// Combines the identities stabby derived for a list of types
///
/// A type that stabby lays out carries [`IStable::ID`], a hash over its
/// report. The report names the type, its module, and the name and type of
/// every field, recursively. This folds those identities into one value,
/// so a boundary made of such types hashes to one number. The length goes
/// in first and the order matters, so two lists holding the same
/// identities in another order hash apart, as do a list and its own
/// prefix.
///
/// The fold is FNV-1a, a handful of const operations. It detects drift
/// between two builds of the same project, and a plugin that misreports
/// its layout can report the host's number.
///
/// # Examples
///
/// ```
/// use stabby::IStable;
/// use whisker_types::plugin::stable_fingerprint;
///
/// const A: u64 = stable_fingerprint(&[<u32 as IStable>::ID]);
/// const B: u64 = stable_fingerprint(&[<u64 as IStable>::ID]);
///
/// assert_ne!(A, B);
/// ```
///
/// [`IStable::ID`]: stabby::IStable::ID
pub const fn stable_fingerprint(ids: &[u64]) -> u64 {
    let mut hash = mix(0xcbf2_9ce4_8422_2325, ids.len() as u64);

    let mut index = 0;
    while index < ids.len() {
        hash = mix(hash, ids[index]);
        index += 1;
    }

    hash
}

/// Folds one value into a running FNV-1a hash, byte by byte
const fn mix(hash: u64, value: u64) -> u64 {
    let bytes = value.to_le_bytes();

    let mut hash = hash;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_fingerprint_distinguishes_a_changed_id() {
        let one = stable_fingerprint(&[1, 2]);

        let other = stable_fingerprint(&[1, 3]);

        assert_ne!(one, other);
    }

    #[test]
    fn stable_fingerprint_of_no_ids_is_not_zero() {
        let empty = stable_fingerprint(&[]);

        assert_ne!(empty, 0);
    }

    #[test]
    fn stable_fingerprint_of_the_same_ids_is_stable() {
        let ids = [1, 2, 3];

        let repeated = stable_fingerprint(&ids);

        assert_eq!(stable_fingerprint(&ids), repeated);
    }

    #[test]
    fn stable_fingerprint_orders_its_ids() {
        let one = stable_fingerprint(&[1, 2]);

        let other = stable_fingerprint(&[2, 1]);

        assert_ne!(one, other);
    }

    #[test]
    fn stable_fingerprint_separates_a_list_from_its_prefix() {
        let short = stable_fingerprint(&[1]);

        let long = stable_fingerprint(&[1, 0]);

        assert_ne!(short, long);
    }
}
