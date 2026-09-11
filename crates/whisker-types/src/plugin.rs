//! Vocabulary for custom lint plugins
//!
//! A custom lint plugin is a dynamic library that whisker compiles and
//! loads at check time, given only a path. Rust has no stable ABI of its
//! own, so stabby lays out every type that crosses the boundary. Those
//! layouts hold under any compiler. A loaded library is coherent with the
//! whisker binary when both lay those types out the same way. That is a
//! property of the source each was built from, not of the rustc that
//! built it. This module defines the declaration a plugin exports and the
//! constants whisker compares to establish exactly that before it trusts
//! anything else in the library.
//!
//! The handshake proceeds in order of decreasing layout stability:
//!
//! 1. [`PluginDeclaration::abi_version`] is a pair of bare integers at
//!    offset zero of a `#[repr(C)]` struct, so it reads correctly
//!    whatever else changed.
//! 2. The two fingerprints are plain integers, readable under any pair
//!    of compilers.
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
//! source they compiled, not the compiler, and not the dependency graph
//! each side resolved. Layout is what unsoundness turns on, and it is far
//! narrower than source text. A doc comment or a private helper moves
//! nothing, so a plugin stays loadable across most of whisker's own churn.
//! A plugin's lockfile picks its own `tree-sitter`. Its `Node` is a
//! `#[repr(transparent)]` wrapper around a `#[repr(C)]` struct of the C
//! library, so a patch-level difference moves no field. It does give each
//! image its own copy of that C library, and a plugin reads a tree the
//! host parsed through its copy. Whisker accepts that residual risk rather
//! than pinning every resolved version a plugin may build against.

use stabby::IStable;

use crate::{
    BoxedLintPass, DecoratedNode, DecorationKey, DecorationLookup, Diagnostic, FilePath, Location,
    Panic, RuleId, RuleOption, RuleOptions, Severity, Span, Suggestion,
};

mod abi_version;
mod declaration;
mod factory;
mod fingerprint;

pub use abi_version::AbiVersion;
pub use declaration::PluginDeclaration;
pub use factory::{Constructed, Factories, LintPassFactory, Loaded, Plugin, construct, factory};
pub use fingerprint::stable_fingerprint;

/// The version of the plugin declaration protocol this crate speaks
///
/// This guards three things no fingerprint can read back: the shape of
/// [`PluginDeclaration`], the meaning of its fields, and the signatures
/// of [`LintPass`]'s methods. The two fingerprints guard everything
/// else. Raise this whenever the declaration struct or [`LintPass`]
/// changes.
///
/// [`AbiVersion`] says which component to raise, and which plugins that
/// leaves behind.
///
/// [`LintPass`]: crate::LintPass
///
/// # Examples
///
/// ```
/// use whisker_types::plugin::{ABI_VERSION, AbiVersion};
///
/// assert_eq!(ABI_VERSION, AbiVersion { major: 0, minor: 1 });
/// ```
pub const ABI_VERSION: AbiVersion = AbiVersion { major: 0, minor: 1 };

/// A fingerprint of how this crate lays out the plugin boundary
///
/// The crate version cannot detect drift, because whisker's crates are
/// unpublished and hold one version between releases. Hashing the source
/// text detects far too much of it: a doc comment or a private helper
/// would refuse every plugin in the tree until each was rebuilt, which is
/// the whole cost of shipping rules as plugins. This hashes what the two
/// images must actually agree on instead.
///
/// Stabby lays out every type that crosses, and each contributes the
/// identity stabby derives from its report. That is a hash over the
/// type's name, its module, and the name and type of every field,
/// recursively. It refuses a field that moved, and a field whose type
/// changed to another of the same size.
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

#[cfg(test)]
mod tests {
    use super::*;

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
