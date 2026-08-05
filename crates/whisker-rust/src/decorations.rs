//! Type information the Rust provider attaches to tree-sitter nodes
//!
//! Every type here crosses from the provider to the lint crates, so none of
//! them names an `ra_ap_*` type. What a lint can ask is exactly what this
//! module can express.

mod adt_flags;
mod fn_signature;
mod resolved_type;

pub use adt_flags::AdtFlags;
pub use fn_signature::FnSignature;
pub use resolved_type::ResolvedType;
