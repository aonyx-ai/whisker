use whisker_types::{DecoratedTree, DecorationProvider};

/// Stub decoration provider for Rust
///
/// This provider does not attach any decorations. A real implementation
/// would connect to rust-analyzer or another toolchain to populate
/// type information, resolution data, and other semantic decorations.
pub struct RustDecorationProvider;

impl DecorationProvider for RustDecorationProvider {
    fn decorate(&self, _tree: &mut DecoratedTree) -> anyhow::Result<()> {
        Ok(())
    }
}
