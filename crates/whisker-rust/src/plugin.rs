//! What a Rust custom lint plugin needs to declare itself
//!
//! This module re-exports the plugin vocabulary from whisker-types and adds
//! the one value only this crate can supply: the fingerprint of the Rust
//! language support a plugin was built against. A plugin crate depends on
//! whisker-rust, implements [`RustLintPass`] for its rules, and hands them
//! to [`export_lints!`], which writes the exported declaration; nothing
//! here needs to be touched by hand.
//!
//! [`RustLintPass`]: crate::RustLintPass
//! [`export_lints!`]: crate::export_lints

use stabby::IStable;
pub use whisker_types::Panic;
use whisker_types::plugin::stable_fingerprint;
pub use whisker_types::plugin::{
    ABI_VERSION, Constructed, Factories, LintPassFactory, Loaded, MIN_ABI_VERSION, Plugin,
    PluginDeclaration, TYPES_FINGERPRINT, factory,
};

use crate::decorations::{AdtFlags, FnSignature, ImportSource, ResolvedType};

include!(concat!(env!("OUT_DIR"), "/visitor_fingerprint.rs"));

/// A fingerprint of the Rust language support a plugin was built against
///
/// Two things on this side shape the boundary. The generated lint pass
/// trait is one: its methods are named after the grammar's node kinds. A
/// plugin generated from a different grammar still loads, and its checks
/// never fire, so [`VISITOR_FINGERPRINT`] hashes the generated source. The decoration types are the other, because a plugin
/// reads them out of a node the host decorated, and reading them at a
/// layout the host did not write is unsound rather than merely quiet.
///
/// Each decoration contributes the identity stabby derives from its
/// report, which covers the name and type of every field, recursively. A
/// type a decoration holds, such as [`TypePath`] inside [`FnSignature`],
/// is therefore covered without being named here. The list names every
/// type that implements [`Decoration`], and a new one belongs on it.
///
/// Everything else in this crate is deliberately absent. Hashing the
/// whole crate refuses every plugin in the tree whenever any source file
/// here changes.
///
/// # Examples
///
/// ```
/// use whisker_rust::plugin::LANGUAGE_FINGERPRINT;
///
/// assert_ne!(LANGUAGE_FINGERPRINT, 0);
/// ```
///
/// [`Decoration`]: whisker_types::Decoration
/// [`TypePath`]: crate::decorations::TypePath
pub const LANGUAGE_FINGERPRINT: u64 = stable_fingerprint(&[
    VISITOR_FINGERPRINT,
    <AdtFlags as IStable>::ID,
    <FnSignature as IStable>::ID,
    <ImportSource as IStable>::ID,
    <ResolvedType as IStable>::ID,
]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fingerprint_covers_the_decorations() {
        let visitor_alone = stable_fingerprint(&[VISITOR_FINGERPRINT]);

        let combined = LANGUAGE_FINGERPRINT;

        assert_ne!(combined, visitor_alone);
        assert_ne!(combined, 0);
    }

    #[test]
    fn visitor_fingerprint_is_not_zero() {
        let generated = VISITOR_FINGERPRINT;

        assert_ne!(generated, 0);
    }
}
