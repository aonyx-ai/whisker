use stabby::IStable;
use whisker_types::plugin::stable_fingerprint;

use crate::decorations::TypeText;

include!(concat!(env!("OUT_DIR"), "/visitor_fingerprint.rs"));

/// A fingerprint of the TypeScript language support a plugin was built against
///
/// whisker-plugin joins it with every other language crate's into the
/// value a plugin's declaration carries.
///
/// Two things on this side shape the boundary. The generated lint pass
/// trait is one: its methods are named after the grammars' node kinds. A
/// plugin generated from a different grammar still loads, and its checks
/// never fire, so [`VISITOR_FINGERPRINT`] hashes the generated source. The
/// decoration types are the other. A plugin reads them out of a node the
/// host decorated, and reading them at another layout is unsound rather
/// than merely quiet. Each contributes the
/// identity stabby derives from its report. The list names every type
/// that implements [`Decoration`], and a new one belongs on it.
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
///
/// [`Decoration`]: whisker_types::Decoration
pub const LANGUAGE_FINGERPRINT: u64 =
    stable_fingerprint(&[VISITOR_FINGERPRINT, <TypeText as IStable>::ID]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fingerprint_covers_the_decorations() {
        let visitor_alone = stable_fingerprint(&[VISITOR_FINGERPRINT]);

        let fingerprint = LANGUAGE_FINGERPRINT;

        assert_ne!(fingerprint, visitor_alone);
        assert_ne!(fingerprint, 0);
    }

    #[test]
    fn visitor_fingerprint_is_not_zero() {
        let generated = VISITOR_FINGERPRINT;

        assert_ne!(generated, 0);
    }
}
