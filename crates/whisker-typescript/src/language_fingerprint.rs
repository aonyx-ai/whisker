use whisker_types::plugin::stable_fingerprint;

include!(concat!(env!("OUT_DIR"), "/visitor_fingerprint.rs"));

/// A fingerprint of the TypeScript language support a plugin was built against
///
/// whisker-plugin joins it with every other language crate's into the
/// value a plugin's declaration carries.
///
/// The generated lint pass trait is what shapes the boundary on this
/// side: its methods are named after the grammars' node kinds. A plugin
/// generated from a different grammar still loads, and its checks never
/// fire, so [`VISITOR_FINGERPRINT`] hashes the generated source.
///
/// Everything else in this crate is deliberately absent. Hashing the
/// whole crate refuses every plugin in the tree whenever any source file
/// here changes.
///
/// # Examples
///
/// ```
/// use whisker_typescript::LANGUAGE_FINGERPRINT;
///
/// assert_ne!(LANGUAGE_FINGERPRINT, 0);
/// ```
pub const LANGUAGE_FINGERPRINT: u64 = stable_fingerprint(&[VISITOR_FINGERPRINT]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fingerprint_covers_the_visitor() {
        let visitor = VISITOR_FINGERPRINT;

        let fingerprint = LANGUAGE_FINGERPRINT;

        assert_ne!(fingerprint, visitor);
        assert_ne!(fingerprint, 0);
    }

    #[test]
    fn visitor_fingerprint_is_not_zero() {
        let generated = VISITOR_FINGERPRINT;

        assert_ne!(generated, 0);
    }
}
