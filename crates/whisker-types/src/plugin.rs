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
//!    [`PluginDeclaration::load`] be called, because what it hands back
//!    is only meaningful once the two images are known to lay it out the
//!    same way.
//!
//! Every value that crosses the boundary is one stabby lays out, and every
//! allocation among them carries the function that frees it, so the side
//! that allocated a value is the side that frees it whatever the other
//! side's allocator is. A plugin may therefore set its own
//! `#[global_allocator]`.
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

use stabby::IStable;

use crate::{
    BoxedLintPass, DecoratedNode, DecorationKey, DecorationLookup, Diagnostic, FilePath, Location,
    Panic, RuleId, RuleOption, RuleOptions, Severity, Span, Suggestion,
};

mod declaration;
mod factory;
mod fingerprint;

pub use declaration::PluginDeclaration;
pub use factory::{Constructed, Factories, LintPassFactory, Loaded, Plugin, construct, factory};
pub use fingerprint::{Shape, fingerprint, seeded_fingerprint, stable_fingerprint};

/// The version of the plugin declaration protocol itself
///
/// This guards the shape of [`PluginDeclaration`] and the meaning of its
/// fields, and the signatures of [`LintPass`]'s methods, which no
/// fingerprint can read back. The rustc version and the two fingerprints
/// guard everything else. Bump it whenever the declaration struct or
/// [`LintPass`] changes.
///
/// [`LintPass`]: crate::LintPass
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::ABI_VERSION;
///
/// assert_eq!(ABI_VERSION, 6);
/// ```
pub const ABI_VERSION: u32 = 6;

/// The oldest protocol whisker still loads
///
/// A protocol is raised when the declaration gains a field, and a plugin
/// written before it simply ends sooner. Whisker knows the layout of
/// every version in this range, so it reads what such a plugin has and
/// treats the rest as absent.
///
/// This range covers the declaration alone. A change to the method list
/// of [`LintPass`] reorders a vtable, which no version can make readable,
/// so such a change raises this floor to meet [`ABI_VERSION`] and refuses
/// everything older. Protocol 6 hands everything a plugin exports over
/// through one `load` function, which no earlier protocol supplies. 6 is
/// therefore the only protocol whisker loads until the declaration next
/// gains a field.
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
pub const MIN_ABI_VERSION: u32 = 6;

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
/// images must actually agree on instead.
///
/// A type that stabby lays out contributes the identity stabby derives
/// from its report. That is a hash over the type's name, its module, and
/// the name and type of every field, recursively. It refuses a field that
/// moved, and a field whose type changed to another of the same size.
///
/// The list names every value that crosses, and nothing else. A pass
/// receives a [`DecoratedNode`], which reaches its file and its
/// decorations through a [`FilePath`] and a [`DecorationLookup`], and the
/// [`RuleOptions`] a project set. It returns [`Diagnostic`]s, or the
/// [`Panic`] that stopped it. A plugin hands its exports over as a
/// [`Plugin`], and each factory builds a [`BoxedLintPass`].
///
/// A stabby result is not on the list, and neither is a stabby list. Both
/// are stabby's own layouts, which the stabby every plugin shares
/// decides, so the list names their payloads instead. The tree, the
/// decoration map, and the coverage types stay on the host's side of the
/// boundary, so they are not here.
///
/// What it does not cover is the signatures of [`LintPass`]'s methods.
/// The factory's identity reaches the vtable and names its methods in
/// order, so a method added, removed, or moved is refused, but the report
/// records each method as a pointer and not what it takes and returns. A
/// changed signature is a change to the protocol and belongs in
/// [`ABI_VERSION`]. The `abi_version_covers_the_boundary_traits` test in
/// this module fails when the trait's method list moves, so the bump is
/// not left to memory.
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
pub const TYPES_FINGERPRINT: u64 = stable_fingerprint(&[
    <DecoratedNode<'static> as IStable>::ID,
    <DecorationLookup<'static> as IStable>::ID,
    <DecorationKey as IStable>::ID,
    <FilePath as IStable>::ID,
    <RuleOptions as IStable>::ID,
    <RuleOption as IStable>::ID,
    <Panic as IStable>::ID,
    <LintPassFactory as IStable>::ID,
    <Plugin as IStable>::ID,
    <BoxedLintPass as IStable>::ID,
    <Diagnostic as IStable>::ID,
    <Span as IStable>::ID,
    <Suggestion as IStable>::ID,
    <Location as IStable>::ID,
    <RuleId as IStable>::ID,
    <Severity as IStable>::ID,
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
            .filter(|line| line.starts_with("fn ") || line.starts_with("extern \"C\" fn "))
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect()
    }

    #[test]
    fn abi_version_covers_the_boundary_traits() {
        let lint_pass = method_signatures(
            include_str!("lint_pass.rs"),
            "pub trait LintPass: Send + Sync {",
        );

        assert_eq!(
            lint_pass,
            vec![
                "extern \"C\" fn configure(&mut self, options: &RuleOptions) -> Configured;"
                    .to_owned(),
                "extern \"C\" fn check_node(&mut self, node: &DecoratedNode<'_>) -> Checked;"
                    .to_owned()
            ],
            "the boundary trait's methods moved, which changes its vtable; \
             bump ABI_VERSION and update this test together",
        );
    }

    #[test]
    fn types_fingerprint_covers_every_boundary_type() {
        let one = stable_fingerprint(&[<Diagnostic as IStable>::ID]);

        let all = TYPES_FINGERPRINT;

        assert_ne!(all, one);
        assert_ne!(all, 0);
    }
}
