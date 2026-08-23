//! Lints that read decorations, so the provider's output stays under test
//!
//! The rules whisker's authors actually use live in their own repository,
//! and a test here cannot depend on them without tying the two together.
//! What those tests were really asserting was never the rules' policy but
//! the provider's: that a real [`FnSignature`], [`ImportSource`],
//! [`ResolvedType`], or [`AdtFlags`] recorded against a real project says
//! what a rule needs it to say. Each probe below carries the part of one
//! rule that reads a decoration, and none of the policy that surrounds it.
//!
//! The probes arrive one at a time; this is the first of them.
//!
//! [`AdtFlags`]: whisker_rust::decorations::AdtFlags
//! [`FnSignature`]: whisker_rust::decorations::FnSignature
//! [`ImportSource`]: whisker_rust::decorations::ImportSource
//! [`ResolvedType`]: whisker_rust::decorations::ResolvedType

mod anyhow_bare_try;

pub use anyhow_bare_try::AnyhowBareTry;

#[cfg(feature = "plugin")]
whisker_rust::export_lints![AnyhowBareTry];
