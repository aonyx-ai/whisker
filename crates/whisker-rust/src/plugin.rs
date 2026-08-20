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

use std::ffi::CStr;

pub use whisker_types::plugin::{
    ABI_VERSION, LintRegistrar, PluginDeclaration, RUSTC_VERSION, TYPES_FINGERPRINT, c_str,
};

/// A fingerprint of the whisker-rust source this crate was compiled from
///
/// The hash covers the crate's sources, the grammar's node types, and the
/// generated lint pass trait, so a plugin built against different Rust
/// language support is refused at load rather than dispatched into
/// methods that no longer line up with the tree.
///
/// # Examples
///
/// ```
/// use whisker_rust::plugin::LANGUAGE_FINGERPRINT;
///
/// let fingerprint = LANGUAGE_FINGERPRINT.to_str().expect("should be UTF-8");
///
/// assert_eq!(fingerprint.len(), 16);
/// ```
pub const LANGUAGE_FINGERPRINT: &CStr = c_str(concat!(env!("WHISKER_RUST_FINGERPRINT"), "\0"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fingerprint_is_a_64_bit_hex_string() {
        let fingerprint = LANGUAGE_FINGERPRINT.to_str().expect("should be UTF-8");

        assert_eq!(fingerprint.len(), 16);
        assert!(fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
