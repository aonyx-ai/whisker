use std::ffi::c_char;

use crate::RuleId;
use crate::plugin::LintRegistrar;

/// The entry point a custom lint plugin exports
///
/// A plugin exports exactly one static of this type under the symbol name
/// `whisker_plugin_declaration`; the `export_lints!` macro in whisker-rust
/// writes it. The loader reads the fields in declaration order and stops at
/// the first mismatch, because each field's readability rests on
/// progressively stronger assumptions:
///
/// - [`abi_version`] is a bare integer at offset zero of a `#[repr(C)]`
///   struct, readable whatever else changed.
/// - [`rustc_version`] is a pointer to a NUL-terminated string in the
///   plugin's immutable data, readable across rustc versions. It is a raw
///   pointer rather than a `&CStr`, because a reference's layout is only
///   promised within one compiler. The two fingerprints are plain `u64`,
///   which needs no such promise.
/// - [`register`] is a plain Rust function pointer. Calling it hands
///   `&mut dyn` trait objects across the boundary, which is only sound
///   once every prior field proved that both images agree on the ABI.
///
/// The fields are public rather than accessed through getters, because
/// this struct is a wire format: the exporting macro constructs it in a
/// `const` context and the loader consumes it field by field.
///
/// `Send` and `Sync` are implemented by hand, because the version pointer
/// makes the type `!Sync` by default; they are sound because it points at
/// immutable `'static` data in the plugin image.
///
/// [`abi_version`]: PluginDeclaration::abi_version
/// [`rustc_version`]: PluginDeclaration::rustc_version
/// [`register`]: PluginDeclaration::register
#[repr(C)]
pub struct PluginDeclaration {
    /// The plugin's copy of [`ABI_VERSION`]
    ///
    /// [`ABI_VERSION`]: crate::plugin::ABI_VERSION
    pub abi_version: u32,

    /// The plugin's copy of [`RUSTC_VERSION`]
    ///
    /// [`RUSTC_VERSION`]: crate::plugin::RUSTC_VERSION
    pub rustc_version: *const c_char,

    /// The plugin's copy of [`TYPES_FINGERPRINT`]
    ///
    /// [`TYPES_FINGERPRINT`]: crate::plugin::TYPES_FINGERPRINT
    pub types_fingerprint: u64,

    /// The plugin's fingerprint of the language crate it was built against
    ///
    /// For Rust lints this is whisker-rust's fingerprint, which also covers
    /// the lint pass trait generated from the grammar's node types. A
    /// plugin whose dispatch was generated from a different grammar would
    /// not crash, but its checks would quietly never fire; the handshake
    /// turns that into a refusal.
    pub language_fingerprint: u64,

    /// Registers the plugin's lint passes with the host
    ///
    /// The host calls this once, after the handshake, with a registrar
    /// that collects one factory per lint.
    pub register: fn(&mut dyn LintRegistrar),

    /// Returns every rule this plugin can report
    ///
    /// A project names the rules it runs, and whisker refuses a name that
    /// no loaded plugin declares. Without this, a misspelled name would
    /// disable nothing and say nothing, which reads exactly like a rule
    /// that found no fault.
    ///
    /// This is the first field a plugin may lack. A plugin built against
    /// protocol 2 ends after `register`, so whisker reads this only once
    /// [`PluginDeclaration::abi_version`] says it is there. Anything
    /// added later goes on the end for the same reason: a `#[repr(C)]`
    /// struct has offsets whisker can reason about, and a vtable does
    /// not, so a capability that some plugins lack belongs here rather
    /// than on [`LintPass`].
    ///
    /// [`LintPass`]: crate::LintPass
    pub rules: fn() -> Vec<RuleId>,
}

unsafe impl Send for PluginDeclaration {}
unsafe impl Sync for PluginDeclaration {}

#[cfg(test)]
mod tests {
    use std::mem::offset_of;

    use super::*;

    #[test]
    fn abi_version_sits_at_offset_zero() {
        assert_eq!(offset_of!(PluginDeclaration, abi_version), 0);
    }

    #[test]
    fn trait_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PluginDeclaration>();
    }

    #[test]
    fn trait_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PluginDeclaration>();
    }

    #[test]
    fn trait_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<PluginDeclaration>();
    }
}
