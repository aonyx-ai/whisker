use crate::plugin::{AbiVersion, Loaded};

/// The entry point a custom lint plugin exports
///
/// A plugin exports exactly one static of this type under the symbol name
/// `whisker_plugin_declaration`; the `export_lints!` macro in whisker-rust
/// writes it. The loader reads the fields in declaration order and stops at
/// the first mismatch, because each field's readability rests on
/// progressively stronger assumptions:
///
/// - [`abi_version`] is a pair of bare integers at offset zero of a
///   `#[repr(C)]` struct, readable whatever else changed.
/// - [`types_fingerprint`] and [`language_fingerprint`] are plain `u64`,
///   readable under any pair of compilers.
/// - [`load`] is an `extern "C"` function pointer that hands back values
///   stabby lays out. Calling it is sound once the fingerprints prove
///   that both images lay those values out the same way.
///
/// The fields are public rather than accessed through getters, because
/// this struct is a wire format: the exporting macro constructs it in a
/// `const` context and the loader consumes it field by field.
///
/// [`abi_version`]: PluginDeclaration::abi_version
/// [`language_fingerprint`]: PluginDeclaration::language_fingerprint
/// [`load`]: PluginDeclaration::load
/// [`types_fingerprint`]: PluginDeclaration::types_fingerprint
#[repr(C)]
pub struct PluginDeclaration {
    /// The plugin's copy of [`ABI_VERSION`]
    ///
    /// [`ABI_VERSION`]: crate::plugin::ABI_VERSION
    pub abi_version: AbiVersion,

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

    /// Hands over everything the plugin exports
    ///
    /// The host calls this once, after the handshake, and keeps what it
    /// gets for the life of the process. One call carries the factories
    /// and the rule list together.
    ///
    /// This is the only function the boundary needs. After `dlopen` the
    /// host holds an address and a name and no stabby value, so the first
    /// hand-over must be a call whose convention both images agree on
    /// with no prior agreement. Everything the call returns is stabby's.
    ///
    /// Anything added later goes inside [`Plugin`] rather than beside it.
    /// A `#[repr(C)]` struct has offsets whisker can reason about per
    /// protocol, and a vtable does not.
    ///
    /// [`Plugin`]: crate::plugin::Plugin
    pub load: extern "C" fn() -> Loaded,
}

#[cfg(test)]
mod tests {
    use std::mem::offset_of;

    use super::*;

    #[test]
    fn abi_version_sits_at_offset_zero() {
        assert_eq!(offset_of!(PluginDeclaration, abi_version), 0);
    }

    /// Pins that the fields keep the order the protocol was written in
    ///
    /// Whisker reads each field at the offset this struct gives. A field
    /// inserted among them would move the rest, and whisker would read one
    /// plugin's data as another field. That is silent: the fields are
    /// integers and pointers, and a wrong one is a wrong answer rather than
    /// a crash. A new field goes last, and this test fails if one does not.
    #[test]
    fn load_sits_after_every_other_field() {
        let load = offset_of!(PluginDeclaration, load);

        assert!(offset_of!(PluginDeclaration, abi_version) < load);
        assert!(offset_of!(PluginDeclaration, types_fingerprint) < load);
        assert!(offset_of!(PluginDeclaration, language_fingerprint) < load);
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
