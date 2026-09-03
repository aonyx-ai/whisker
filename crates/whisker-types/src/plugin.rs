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
//! 2. The rustc version is a C string and the two fingerprints are plain
//!    integers, all readable across any pair of rustc versions.
//! 3. Only when every one of them matches the host's own constants may
//!    [`PluginDeclaration::register`] be called, because a plain Rust
//!    function pointer is only meaningful once the two images are known to
//!    agree on the language's ABI.
//!
//! A plugin must not set its own `#[global_allocator]`: the host frees the
//! values a plugin allocated (diagnostics, boxed passes), which is sound
//! because both images default to the system allocator.
//!
//! The handshake reaches how both sides lay out the boundary, not the
//! source they compiled and not the dependency graph each side resolved.
//! Layout is what unsoundness turns on, and it is far narrower than source
//! text: a doc comment or a private helper moves nothing, so a plugin
//! stays loadable across most of whisker's own churn. A plugin's lockfile
//! picks its own
//! `tree-sitter`, whose `Node` is a `#[repr(transparent)]` wrapper around
//! a `#[repr(C)]` struct of the C library, so a patch-level difference
//! moves no field. It does give each image its own copy of that C
//! library, and a plugin reads a tree the host parsed through its copy.
//! Whisker accepts that residual risk rather than pinning every resolved
//! version a plugin may build against.

use std::ffi::CStr;

use crate::{
    Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationKey, DecorationMap, Diagnostic,
    Language, Location, ProviderName, RuleId, Severity, Span, Suggestion, UncoveredFile,
};

mod declaration;
mod fingerprint;
mod registrar;

pub use declaration::PluginDeclaration;
pub use fingerprint::{Shape, fingerprint, seeded_fingerprint};
pub use registrar::{LintPassFactory, LintRegistrar};

/// The version of the plugin declaration protocol itself
///
/// This guards the shape of [`PluginDeclaration`] and the meaning of its
/// fields, and the method order of the two traits that cross the boundary,
/// which no fingerprint can read back. The rustc version and the two
/// fingerprints guard everything else. Bump it whenever the declaration
/// struct, the registration contract, [`LintPass`], or [`LintRegistrar`]
/// changes.
///
/// [`LintPass`]: crate::LintPass
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::ABI_VERSION;
///
/// assert_eq!(ABI_VERSION, 3);
/// ```
pub const ABI_VERSION: u32 = 3;

/// The oldest protocol whisker still loads
///
/// A protocol is raised when the declaration gains a field, and a plugin
/// written before it simply ends sooner. Whisker knows the layout of
/// every version in this range, so it reads what such a plugin has and
/// treats the rest as absent.
///
/// This range covers the declaration alone. A change to the method list
/// of [`LintPass`] or [`LintRegistrar`] reorders a vtable, which no
/// version can make readable, so such a change raises this floor to meet
/// [`ABI_VERSION`] and refuses everything older.
///
/// [`LintPass`]: crate::LintPass
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::{ABI_VERSION, MIN_ABI_VERSION};
///
/// assert!(MIN_ABI_VERSION <= ABI_VERSION);
/// ```
pub const MIN_ABI_VERSION: u32 = 2;

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

/// A fingerprint of how this crate lays out the plugin boundary
///
/// The crate version cannot detect drift, because whisker's crates are
/// unpublished and hold one version between releases. Hashing the source
/// text detects far too much of it: a doc comment or a private helper
/// would refuse every plugin in the tree until each was rebuilt, which is
/// the whole cost of shipping rules as plugins. This hashes what the two
/// images must actually agree on instead — the size, alignment, and field
/// offsets of every type that crosses the boundary.
///
/// What it does not cover is the shape of [`LintPass`] and
/// [`LintRegistrar`] themselves. A trait object's vtable orders its
/// methods by declaration, and no const can read that back, so adding,
/// removing, or reordering a method on either trait is a change to the
/// protocol and belongs in [`ABI_VERSION`]. The `abi_version_covers_the
/// _boundary_traits` test in this module fails when either trait's method
/// list moves, so the bump is not left to memory.
///
/// [`LintPass`]: crate::LintPass
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::TYPES_FINGERPRINT;
///
/// assert_ne!(TYPES_FINGERPRINT, 0);
/// ```
pub const TYPES_FINGERPRINT: u64 = fingerprint(&[
    Shape::of_fields::<Diagnostic>(crate::diagnostic::FIELD_OFFSETS),
    Shape::of_fields::<Span>(crate::span::FIELD_OFFSETS),
    Shape::of_fields::<Suggestion>(crate::suggestion::FIELD_OFFSETS),
    Shape::of_fields::<Location>(crate::location::FIELD_OFFSETS),
    Shape::of_fields::<DecoratedNode<'static>>(crate::decorated_node::FIELD_OFFSETS),
    Shape::of::<DecoratedTree>(),
    Shape::of::<DecorationKey>(),
    Shape::of::<DecorationMap>(),
    Shape::of::<RuleId>(),
    Shape::of::<Severity>(),
    Shape::of::<Language>(),
    Shape::of::<ProviderName>(),
    Shape::of::<Coverage>(),
    Shape::of::<CoverageGap>(),
    Shape::of::<UncoveredFile>(),
    Shape::of::<LintPassFactory>(),
]);

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

    /// Returns each method signature a trait declares, in declaration order
    ///
    /// The scan takes the lines inside the trait's block that open a
    /// method and squeezes their whitespace, so a doc comment or a
    /// reflowed line changes nothing while an added, removed, reordered,
    /// or re-signed method does.
    fn method_signatures(source: &str, declaration: &str) -> Vec<String> {
        let body = source
            .split_once(declaration)
            .expect("the trait should be declared in this source")
            .1;
        let body = body
            .split_once("\n}")
            .expect("the trait block should be closed")
            .0;

        body.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("fn "))
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect()
    }

    #[test]
    fn abi_version_covers_the_boundary_traits() {
        let lint_pass = method_signatures(
            include_str!("lint_pass.rs"),
            "pub trait LintPass: Send + Sync {",
        );
        let registrar = method_signatures(
            include_str!("plugin/registrar.rs"),
            "pub trait LintRegistrar {",
        );

        assert_eq!(
            (lint_pass, registrar),
            (
                vec![
                    "fn check_node(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic>;"
                        .to_owned()
                ],
                vec!["fn register(&mut self, factory: LintPassFactory);".to_owned()],
            ),
            "a boundary trait's methods moved, which reorders its vtable; \
             bump ABI_VERSION and update this test together",
        );
    }

    #[test]
    fn types_fingerprint_covers_every_boundary_type() {
        let one = fingerprint(&[Shape::of::<Diagnostic>()]);

        let all = TYPES_FINGERPRINT;

        assert_ne!(all, one);
        assert_ne!(all, 0);
    }
}
