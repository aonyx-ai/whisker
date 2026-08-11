//! Holds code that no crate's module tree reaches
//!
//! Cargo never compiles this file, because it sits beside `Cargo.toml` and
//! outside the crate's `src` directory. rust-analyzer's VFS still interns
//! it, because the package directory is a source root. Whisker once
//! reported such a file clean, although no lint rule resolved anything.

pub fn orphan() -> i32 {
    42
}
