//! Vocabulary for custom lint plugins
//!
//! A custom lint plugin is a dynamic library that whisker compiles and
//! loads at check time, given only a path. Rust has no stable ABI, so a
//! loaded library is only coherent with the whisker binary when both were
//! compiled by the same rustc from the same source for every type that
//! crosses the boundary. This module defines the declaration a plugin
//! exports and the constants whisker compares to establish exactly that
//! before it trusts anything else in the library.
//!
//! The handshake proceeds in order of decreasing layout stability:
//!
//! 1. [`PluginDeclaration::abi_version`] sits first in a `#[repr(C)]`
//!    struct, so it reads correctly whatever else changed.
//! 2. The version and fingerprint fields are C strings, readable across
//!    any pair of rustc versions.
//! 3. Only when every one of them matches the host's own constants may
//!    [`PluginDeclaration::register`] be called, because a plain Rust
//!    function pointer is only meaningful once the two images are known to
//!    agree on the language's ABI.
//!
//! A plugin must not set its own `#[global_allocator]`: the host frees the
//! values a plugin allocated (diagnostics, boxed passes), which is sound
//! because both images default to the system allocator.
//!
//! The handshake reaches the whisker source both sides compiled, not the
//! dependency graph each side resolved. A plugin's lockfile picks its own
//! `tree-sitter`, whose `Node` is a `#[repr(transparent)]` wrapper around
//! a `#[repr(C)]` struct of the C library, so a patch-level difference
//! moves no field. It does give each image its own copy of that C
//! library, and a plugin reads a tree the host parsed through its copy.
//! Whisker accepts that residual risk rather than pinning every resolved
//! version a plugin may build against.

use std::ffi::CStr;

mod declaration;
mod registrar;

pub use declaration::PluginDeclaration;
pub use registrar::LintRegistrar;

/// The version of the plugin declaration protocol itself
///
/// This guards the shape of [`PluginDeclaration`] and the meaning of its
/// fields, not the ABI of the lint passes; the version and fingerprint
/// strings guard those. Bump it whenever the declaration struct or the
/// registration contract changes.
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::ABI_VERSION;
///
/// assert_eq!(ABI_VERSION, 1);
/// ```
pub const ABI_VERSION: u32 = 1;

/// The full identity of the rustc that compiled this crate
///
/// The plugin loader compares the plugin's copy against the host's. The
/// string carries the commit hash and date, so two nightlies of the same
/// semantic version do not pass for one another.
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::RUSTC_VERSION;
///
/// let version = RUSTC_VERSION.to_str().expect("should be UTF-8");
///
/// assert!(version.starts_with("rustc"));
/// ```
pub const RUSTC_VERSION: &CStr = c_str(concat!(env!("WHISKER_RUSTC_VERSION"), "\0"));

/// A fingerprint of the whisker-types source this crate was compiled from
///
/// The crate version cannot detect drift, because whisker's crates are
/// unpublished and hold one version between releases. The build script
/// hashes every source file instead, so a plugin built against a different
/// revision of whisker-types is refused rather than trusted to share
/// layouts it may not share.
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::TYPES_FINGERPRINT;
///
/// let fingerprint = TYPES_FINGERPRINT.to_str().expect("should be UTF-8");
///
/// assert_eq!(fingerprint.len(), 16);
/// ```
pub const TYPES_FINGERPRINT: &CStr = c_str(concat!(env!("WHISKER_TYPES_FINGERPRINT"), "\0"));

/// Converts a NUL-terminated string literal into a [`&CStr`] at compile time
///
/// Language crates use this for their own handshake constants, the way
/// [`LANGUAGE_FINGERPRINT`] in whisker-rust does.
///
/// # Panics
///
/// Panics at compile time if `text` contains an interior NUL byte or does
/// not end with one.
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::c_str;
///
/// const GREETING: &std::ffi::CStr = c_str("hello\0");
/// ```
///
/// [`&CStr`]: std::ffi::CStr
/// [`LANGUAGE_FINGERPRINT`]: PluginDeclaration::language_fingerprint
pub const fn c_str(text: &'static str) -> &'static CStr {
    match CStr::from_bytes_with_nul(text.as_bytes()) {
        Ok(text) => text,
        Err(_) => panic!("the string must end with exactly one NUL byte"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rustc_version_names_the_compiler() {
        let version = RUSTC_VERSION.to_str().expect("should be UTF-8");

        assert!(version.starts_with("rustc"), "unexpected: {version}");
    }

    #[test]
    fn types_fingerprint_is_a_64_bit_hex_string() {
        let fingerprint = TYPES_FINGERPRINT.to_str().expect("should be UTF-8");

        assert_eq!(fingerprint.len(), 16);
        assert!(fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
