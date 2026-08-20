//! Type information the Rust provider attaches to tree-sitter nodes
//!
//! Every type here crosses from the provider to the lint crates, so none of
//! them names an `ra_ap_*` type. What a lint can ask is exactly what this
//! module can express.

mod adt_flags;
mod error_type;
mod fn_signature;
mod import_source;
mod resolved_type;
mod return_mode;
mod type_path;
mod type_path_ref;

pub use adt_flags::AdtFlags;
pub use error_type::ErrorType;
pub use fn_signature::FnSignature;
pub use import_source::ImportSource;
pub use resolved_type::ResolvedType;
pub use return_mode::ReturnMode;
pub use type_path::TypePath;
pub use type_path_ref::TypePathRef;
