//! What a custom lint plugin needs to declare itself
//!
//! A plugin is a `cdylib` that whisker loads into its own process. Its
//! rules implement a language crate's lint pass trait, such as
//! [`RustLintPass`], and this crate hands them to whisker. [`export_lints!`]
//! writes the declaration whisker looks up, and [`DeclaresRules`] says
//! which rule names a pass reports.
//!
//! The crate also owns [`LANGUAGE_FINGERPRINT`], which joins the
//! fingerprints of every language crate. One plugin may export passes of
//! several languages, and whisker loads it only when every language it
//! could have been built against matches the host's. Keeping that value
//! here, rather than in a language crate, means a plugin that names one
//! language still carries the fingerprint of all of them.
//!
//! [`RustLintPass`]: whisker_rust::RustLintPass

mod declares_rules;
mod export_lints;

pub use declares_rules::DeclaresRules;
pub use whisker_types::Panic;
use whisker_types::plugin::stable_fingerprint;
pub use whisker_types::plugin::{
    ABI_VERSION, AbiVersion, Constructed, Factories, LintPassFactory, Loaded, Plugin,
    PluginDeclaration, TYPES_FINGERPRINT, factory,
};

#[doc(hidden)]
pub mod __private {
    pub use whisker_rust;
}

/// A fingerprint of the language support a plugin was built against
///
/// Each language crate fingerprints the part of it that shapes the
/// boundary: its generated lint pass trait and its decoration types. This
/// constant joins them in a fixed order. A plugin carries it in its
/// declaration, and whisker refuses a plugin whose value differs from its
/// own. A pass generated from another grammar would load and never fire,
/// and a decoration read at another layout is unsound.
///
/// # Examples
///
/// ```
/// use whisker_plugin::LANGUAGE_FINGERPRINT;
///
/// assert_ne!(LANGUAGE_FINGERPRINT, 0);
/// ```
pub const LANGUAGE_FINGERPRINT: u64 = stable_fingerprint(&[whisker_rust::LANGUAGE_FINGERPRINT]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_fingerprint_covers_rust() {
        let rust = whisker_rust::LANGUAGE_FINGERPRINT;

        let joined = LANGUAGE_FINGERPRINT;

        assert_ne!(joined, rust);
        assert_ne!(joined, 0);
    }
}
