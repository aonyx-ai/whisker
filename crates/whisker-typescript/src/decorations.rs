//! Type information the TypeScript provider attaches to tree-sitter nodes
//!
//! Every type here crosses from the provider to the lint crates, so none of
//! them names a tsrs type. What a lint can ask is exactly what this module
//! can express.

mod type_text;

pub use type_text::TypeText;
