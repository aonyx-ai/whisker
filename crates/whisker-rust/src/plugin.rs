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

pub use whisker_types::plugin::{
    ABI_VERSION, LintPassFactory, LintRegistrar, MIN_ABI_VERSION, PluginDeclaration, RUSTC_VERSION,
    TYPES_FINGERPRINT, c_str,
};
use whisker_types::plugin::{Shape, seeded_fingerprint};

use crate::decorations::{
    AdtFlags, ErrorType, FnSignature, ResolvedType, ReturnMode, TypePath, TypePathRef,
};

include!(concat!(env!("OUT_DIR"), "/visitor_fingerprint.rs"));

/// A fingerprint of the Rust language support a plugin was built against
///
/// Two things on this side shape the boundary. The generated lint pass
/// trait is one: its methods are named after the grammar's node kinds and
/// a plugin generated from a different grammar would not crash, its checks
/// would quietly never fire, so [`VISITOR_FINGERPRINT`] hashes the
/// generated source. The decoration types are the other, because a plugin
/// reads them out of a node the host decorated, and reading them at a
/// layout the host did not write is unsound rather than merely quiet.
///
/// Everything else in this crate is deliberately absent. Hashing it, as an
/// earlier revision did, refused every plugin in the tree whenever any
/// source file here changed at all.
///
/// # Examples
///
/// ```
/// use whisker_rust::plugin::LANGUAGE_FINGERPRINT;
///
/// assert_ne!(LANGUAGE_FINGERPRINT, 0);
/// ```
pub const LANGUAGE_FINGERPRINT: u64 = seeded_fingerprint(
    VISITOR_FINGERPRINT,
    &[
        Shape::of::<AdtFlags>(),
        Shape::of::<ErrorType>(),
        Shape::of::<FnSignature>(),
        Shape::of::<ResolvedType>(),
        Shape::of::<ReturnMode>(),
        Shape::of::<TypePath>(),
        Shape::of::<TypePathRef>(),
    ],
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fingerprint_covers_more_than_the_visitor() {
        let visitor_only = VISITOR_FINGERPRINT;

        let combined = LANGUAGE_FINGERPRINT;

        assert_ne!(combined, visitor_only);
        assert_ne!(combined, 0);
    }

    #[test]
    fn visitor_fingerprint_is_not_zero() {
        let generated = VISITOR_FINGERPRINT;

        assert_ne!(generated, 0);
    }
}
